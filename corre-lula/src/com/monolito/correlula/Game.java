package com.monolito.correlula;

import java.util.Random;

/** Estados, camera, desenho e interface. Independente de Android (testavel no desktop). */
final class Game implements World.Listener {
    interface Store {
        int get(String k, int def);
        void put(String k, int v);
    }

    interface Sound {
        void play(int id, float vol, float rate);
        void setMuted(boolean muted);
        void setMusic(boolean on);
    }

    static final int MENU = 0, PLAY = 1, PAUSE = 2, OVER = 3, SHOP = 4, COUNT = 5;
    static final int T_DOWN = 0, T_MOVE = 1, T_UP = 2, T_BACK = 3, T_KEY = 4;

    final Store store;
    final Sound sound;
    Gfx g;
    final Font font = new Font();
    private boolean fontBuilt;
    Batch batch;
    final Models models = new Models();
    final MeshBuilder mb = new MeshBuilder();
    final Scenery scenery = new Scenery();
    final World world = new World();
    int tex;
    int sw = 1, sh = 1;
    float VW = 720f, VH = 1280f;

    int state = MENU;
    float stateT, time;
    private final Random rnd = new Random();

    // persistencia
    int best, bank, skin, skinsOwned, banked;
    final int[] lv = new int[4];
    boolean muted, tutorialPending, newRecord;
    private int menuSub;

    // camera
    private final float[] proj = new float[16], view = new float[16], vp = new float[16], ui = new float[16];
    private final float[] m0 = new float[16], m1 = new float[16], m2 = new float[16];
    private float ex = 1f, ey = 1.7f, ez = 4.3f, cx, cy = 1.2f, cz;
    private float camLX, camGround, camDuck, introT, shakeT;
    private float lulaYaw = (float) Math.PI;

    // interface imediata
    private boolean tapPending;
    private float tapX, tapY;
    private float gx, gy;
    private boolean down, swiped;

    // fila de entrada (thread da UI -> thread GL)
    private final int[] qt = new int[128];
    private final float[] qx = new float[128], qy = new float[128];
    private int qn;

    // popups e fala
    private final String[] popText = new String[3];
    private final float[] popT = new float[3];
    private final int[] popCol = new int[3];
    private String sayText;
    private float sayT;
    private float overDelay;
    private boolean overSoundPlayed;
    private String overTitle = "";
    private float reviveT;

    // particulas
    private static final int NP = 48;
    private final float[] ptX = new float[NP], ptY = new float[NP], ptZ = new float[NP], ptVY = new float[NP], ptT = new float[NP];
    private final int[] ptC = new int[NP];

    private final StringBuilder sb = new StringBuilder(32);

    static final float FOG0 = 45f, FOG1 = 140f;
    static float CAM_Y = 3.6f, CAM_Z = 4.6f, CAM_TY = -3.0f, CAM_TZ = -12f, HFOV = 60f;
    static final float SKY_R = 0.80f, SKY_G = 0.89f, SKY_B = 0.97f;

    Game(Store store, Sound sound) {
        this.store = store;
        this.sound = sound;
        world.listener = this;
        best = store.get("best", 0);
        bank = store.get("bank", 0);
        skin = store.get("skin", 0);
        skinsOwned = store.get("skins", 1) | 1;
        for (int i = 0; i < 4; i++) lv[i] = store.get("lv" + i, 0);
        muted = store.get("mute", 0) == 1;
        tutorialPending = store.get("tut", 0) == 0;
        sound.setMuted(muted);
        enterMenu();
    }

    // ================================================================ ciclo de vida GL
    void surfaceCreated(Gfx gfx, Font.Painter painter) {
        g = gfx;
        if (!fontBuilt) {
            font.build(painter);
            fontBuilt = true;
        }
        tex = g.createTexture(font.pixels, font.aw, font.ah);
        batch = new Batch(g, font);
        models.build(g, mb);
        scenery.invalidate(g, true);
    }

    void surfaceChanged(int w, int h) {
        sw = Math.max(1, w);
        sh = Math.max(1, h);
        VW = 720f;
        VH = VW * sh / sw;
        g.viewport(sw, sh);
    }

    // ================================================================ entrada
    synchronized void input(int type, float nx, float ny) {
        if (qn < qt.length) {
            qt[qn] = type;
            qx[qn] = nx;
            qy[qn] = ny;
            qn++;
        }
    }

    void pauseFromSystem() {
        input(T_KEY, 99, 0);
    }

    private final int[] lt = new int[128];
    private final float[] lx = new float[128], ly = new float[128];

    private void processInput() {
        int n;
        synchronized (this) {
            n = qn;
            System.arraycopy(qt, 0, lt, 0, n);
            System.arraycopy(qx, 0, lx, 0, n);
            System.arraycopy(qy, 0, ly, 0, n);
            qn = 0;
        }
        for (int i = 0; i < n; i++) {
            float x = lx[i] * VW, y = ly[i] * VH;
            switch (lt[i]) {
                case T_DOWN:
                    down = true;
                    swiped = false;
                    gx = x;
                    gy = y;
                    break;
                case T_MOVE:
                    if (down && !swiped) checkSwipe(x, y);
                    break;
                case T_UP:
                    if (down && !swiped) checkSwipe(x, y);
                    if (down && !swiped) {
                        tapPending = true;
                        tapX = x;
                        tapY = y;
                    }
                    down = false;
                    break;
                case T_BACK:
                    back();
                    break;
                case T_KEY: {
                    int k = (int) lx[i];
                    if (k == 99) {
                        if (state == PLAY || state == COUNT) setState(PAUSE);
                    } else if (k == 50) {
                        // enter / espaco
                        if (state == MENU) startRun();
                        else if (state == PAUSE) resume();
                        else if (state == OVER && overDelay <= 0) restart();
                        else if (state == PLAY) world.swipe(World.SW_UP);
                    } else if (state == PLAY) {
                        world.swipe(k);
                    }
                    break;
                }
                default:
                    break;
            }
        }
    }

    private void checkSwipe(float x, float y) {
        float dx = x - gx, dy = y - gy;
        float th = 30f;
        if (dx * dx + dy * dy < th * th) return;
        swiped = true;
        int dir = Math.abs(dx) > Math.abs(dy) ? (dx < 0 ? World.SW_LEFT : World.SW_RIGHT) : (dy < 0 ? World.SW_UP : World.SW_DOWN);
        if (state == PLAY) world.swipe(dir);
    }

    private void back() {
        switch (state) {
            case PLAY:
            case COUNT:
                setState(PAUSE);
                break;
            case PAUSE:
                resume();
                break;
            case SHOP:
                setState(MENU);
                break;
            case OVER:
                if (overDelay <= 0) finishOver(true);
                break;
            default:
                if (exitHook != null) exitHook.run();
                break;
        }
    }

    Runnable exitHook;

    // ================================================================ estados
    private void setState(int s) {
        state = s;
        stateT = 0;
    }

    private void enterMenu() {
        long seed = rnd.nextLong();
        world.reset(seed, tutorialPending);
        world.powDur[0] = 10f + lv[0] * 1.5f;
        world.powDur[1] = 8f + lv[1] * 1.5f;
        world.powDur[2] = 10f + lv[2] * 1.5f;
        world.powDur[3] = 12f + lv[3] * 1.5f;
        world.pregen();
        if (g != null) scenery.invalidate(g, false);
        scenery.seed = seed;
        menuSub = rnd.nextInt(Memes.MENU_SUB.length);
        lulaYaw = (float) Math.PI;
        for (int i = 0; i < 3; i++) popT[i] = 0;
        sayT = 0;
        for (int i = 0; i < NP; i++) ptT[i] = 0;
        setState(MENU);
        sound.setMusic(true);
    }

    private void startRun() {
        introT = 0.8f;
        banked = 0;
        newRecord = false;
        world.started = true;
        setState(PLAY);
        sound.play(Synth.CLICK, 0.6f, 1f);
        say(pick(Memes.START));
    }

    private void restart() {
        finishOver(false);
        startRun();
    }

    private void resume() {
        setState(COUNT);
    }

    private void finishOver(boolean toMenu) {
        commitRun();
        if (tutorialPending) {
            tutorialPending = false;
            store.put("tut", 1);
        }
        enterMenu();
        if (!toMenu) {
            // pula a camera do menu
            ex = 0; ey = 4.4f; ez = 7.2f; cx = 0; cy = 0.4f; cz = -10f;
            lulaYaw = 0;
        }
    }

    private void commitRun() {
        int add = world.coinsRun - banked;
        if (add > 0) {
            bank += add;
            banked = world.coinsRun;
            store.put("bank", bank);
        }
        int sc = (int) world.score;
        if (sc > best) {
            best = sc;
            store.put("best", best);
        }
    }

    private static String pick(String[] a) {
        return a[(int) (Math.random() * a.length)];
    }

    private void say(String s) {
        if (s == null) return;
        sayText = s;
        sayT = 2.4f;
    }

    private void popup(String s, int col) {
        if (s == null) return;
        int k = 0;
        float mt = 9;
        for (int i = 0; i < 3; i++) if (popT[i] < mt) { mt = popT[i]; k = i; }
        popText[k] = s;
        popT[k] = 1.6f;
        popCol[k] = col;
    }

    @Override
    public void onEvent(int ev, int arg) {
        switch (ev) {
            case World.EV_COIN:
                sound.play(Synth.COIN, 0.5f, 1f + Math.min(arg % 20, 10) * 0.012f);
                spark(world.px, world.py + 1f, 0f, 0xFFFFD34D);
                break;
            case World.EV_JUMP: sound.play(Synth.JUMP, 0.55f, 1f); break;
            case World.EV_SUPERJUMP: sound.play(Synth.SUPERJUMP, 0.6f, 1f); break;
            case World.EV_SLIDE: sound.play(Synth.SLIDE, 0.6f, 1f); break;
            case World.EV_LANE: sound.play(Synth.LANE, 0.35f, 1f); break;
            case World.EV_LAND: sound.play(Synth.LAND, 0.5f, 1f); break;
            case World.EV_BUMP: sound.play(Synth.LAND, 0.4f, 1.4f); shakeT = 0.12f; break;
            case World.EV_ONBUS:
                if (Math.random() < 0.3) say(pick(Memes.BUS));
                break;
            case World.EV_STUMBLE:
                sound.play(Synth.STUMBLE, 0.8f, 1f);
                sound.play(Synth.QUACK, 0.7f, 1f);
                shakeT = 0.3f;
                say(pick(Memes.STUMBLE));
                break;
            case World.EV_CRASH:
                sound.play(Synth.CRASH, 0.9f, 1f);
                shakeT = 0.5f;
                onDeath();
                break;
            case World.EV_CAUGHT:
                sound.play(Synth.STUMBLE, 0.8f, 1f);
                sound.play(Synth.QUACK, 0.9f, 0.9f);
                shakeT = 0.4f;
                onDeath();
                break;
            case World.EV_POWER:
                sound.play(Synth.POWER, 0.7f, 1f);
                popup(Memes.POW_NAME[arg], 0xFFFFD60A);
                say(Memes.POW_SAY[arg]);
                break;
            case World.EV_POWER_END:
                sound.play(Synth.LAND, 0.4f, 0.7f);
                break;
            case World.EV_MILESTONE: {
                sound.play(Synth.MILESTONE, 0.7f, 1f);
                String m = Memes.meta(arg);
                popup(m != null ? m : arg + " METROS!", 0xFF7CFC8A);
                String s = Memes.metaSay(arg);
                if (s != null) say(s);
                break;
            }
            case World.EV_COINS100:
                popup(arg + " PICANHAS!", 0xFFFFB4A2);
                say(pick(Memes.COINS));
                break;
            default:
                break;
        }
    }

    private void onDeath() {
        int sc = (int) world.score;
        if (sc > best && sc > 300) newRecord = true;
        commitRun();
        overDelay = 1.15f;
        overSoundPlayed = false;
        overTitle = world.cause == World.CAUSE_CAUGHT ? pick(Memes.CAUGHT_TITLE) : pick(Memes.CRASH_TITLE);
        reviveT = 6f;
        setState(OVER);
    }

    private void spark(float x, float y, float z, int col) {
        int made = 0;
        for (int i = 0; i < NP && made < 3; i++) {
            if (ptT[i] > 0) continue;
            ptX[i] = x + (rnd.nextFloat() - 0.5f) * 0.6f;
            ptY[i] = y + (rnd.nextFloat() - 0.5f) * 0.6f;
            ptZ[i] = z - rnd.nextFloat() * 0.5f;
            ptVY[i] = 1.5f + rnd.nextFloat() * 2f;
            ptT[i] = 0.45f;
            ptC[i] = col;
            made++;
        }
    }

    // ================================================================ quadro
    void frame(float dt) {
        if (dt > 0.1f) dt = 0.1f;
        time += dt;
        stateT += dt;
        processInput();
        update(dt);
        render(dt);
        tapPending = false;
    }

    private void update(float dt) {
        switch (state) {
            case PLAY:
                world.update(dt);
                if (introT > 0) introT -= dt;
                break;
            case COUNT:
                if (stateT >= 3f) setState(PLAY);
                break;
            case OVER:
                world.update(dt);
                if (overDelay > 0) {
                    overDelay -= dt;
                    if (overDelay <= 0.35f && !overSoundPlayed) {
                        overSoundPlayed = true;
                        sound.play(Synth.GAMEOVER, 0.7f, 1f);
                    }
                } else if (reviveT > 0) {
                    reviveT -= dt;
                }
                break;
            default:
                break;
        }
        for (int i = 0; i < 3; i++) if (popT[i] > 0) popT[i] -= dt;
        if (sayT > 0) sayT -= dt;
        if (shakeT > 0) shakeT -= dt;
        for (int i = 0; i < NP; i++) {
            if (ptT[i] <= 0) continue;
            ptT[i] -= dt;
            ptY[i] += ptVY[i] * dt;
            ptZ[i] += (state == PLAY ? 0f : 0f);
        }
        float target = (state == MENU || state == SHOP) ? (float) Math.PI : 0f;
        lulaYaw += (target - lulaYaw) * Math.min(1f, dt * 9f);
    }

    // ================================================================ camera
    private void camera(float dt) {
        World w = world;
        float dex, dey, dez, dcx, dcy, dcz;
        camLX += (w.px * 0.82f - camLX) * Math.min(1f, dt * 9f);
        float gl = w.jet() ? w.py - 0.4f : Math.min(w.surf, w.py);
        camGround += (gl - camGround) * Math.min(1f, dt * 4f);
        float air = Math.max(0f, w.py - gl);
        if (state == MENU) {
            dex = 0.9f; dey = 1.75f; dez = 4.3f; dcx = 0.1f; dcy = 1.25f; dcz = 0f;
        } else if (state == SHOP) {
            dex = 0.4f; dey = 1.3f; dez = 6.5f; dcx = 0f; dcy = -3.6f; dcz = 0f;
        } else if (state == OVER && overDelay < 0.9f) {
            dex = w.px + 2.0f; dey = w.py + 2.4f; dez = -5.2f; dcx = w.px; dcy = w.py + 0.7f; dcz = 0.8f;
        } else {
            float duckF = World.clamp((9f - w.duckD) / 6f, 0f, 1f);
            if (!w.alive) duckF = 0f;
            camDuck += (duckF - camDuck) * Math.min(1f, dt * 3f);
            dex = camLX; dey = CAM_Y + camGround * 0.9f + air * 0.2f + camDuck * 1.1f; dez = CAM_Z + camDuck * 2.2f;
            dcx = camLX; dcy = dey + CAM_TY - camDuck * 0.5f; dcz = CAM_TZ;
        }
        float k;
        if ((state == PLAY || state == PAUSE || state == COUNT) && introT <= 0) k = 1f;
        else if (state == OVER) k = Math.min(1f, dt * (overDelay < 0.9f ? 2.2f : 8f));
        else k = Math.min(1f, dt * 4.5f);
        ex += (dex - ex) * k; ey += (dey - ey) * k; ez += (dez - ez) * k;
        cx += (dcx - cx) * k; cy += (dcy - cy) * k; cz += (dcz - cz) * k;
        float sx = 0, sy = 0;
        if (shakeT > 0) {
            float a = shakeT * 0.35f;
            sx = (rnd.nextFloat() - 0.5f) * a;
            sy = (rnd.nextFloat() - 0.5f) * a;
        }
        float aspect = sw / (float) sh;
        float hfov = HFOV * (float) Math.PI / 180f;
        float vfov = 2f * (float) Math.atan(Math.tan(hfov * 0.5f) / aspect);
        if (vfov > 1.85f) vfov = 1.85f;
        if (vfov < 0.9f) vfov = 0.9f;
        Mat.perspective(proj, vfov, aspect, 0.3f, 430f);
        Mat.lookAt(view, ex + sx, ey + sy, ez, cx + sx, cy + sy, cz);
        Mat.mul(vp, proj, view);
    }

    /** Projeta ponto do mundo para a tela virtual; retorna false se atras da camera. */
    private final float[] pj = new float[2];

    private boolean project(float x, float y, float z) {
        float[] m = vp;
        float cxp = m[0] * x + m[4] * y + m[8] * z + m[12];
        float cyp = m[1] * x + m[5] * y + m[9] * z + m[13];
        float cw = m[3] * x + m[7] * y + m[11] * z + m[15];
        if (cw < 0.1f) return false;
        pj[0] = (cxp / cw * 0.5f + 0.5f) * VW;
        pj[1] = (1f - (cyp / cw * 0.5f + 0.5f)) * VH;
        return true;
    }

    // ================================================================ desenho
    private void render(float dt) {
        World w = world;
        double absP = w.P + (double) w.rebase;
        scenery.update(g, mb, absP);
        camera(dt);

        g.clear(SKY_R, SKY_G, SKY_B);
        // ceu
        Mat.ortho(ui, 0, VW, VH, 0, -1, 1);
        g.state(false, false, true, false);
        g.texPass(ui, tex, 1, 0, 0, 0, 0);
        float hz = project(ex, 0f, ez - 400f) ? pj[1] : VH * 0.3f;
        batch.gradient(0, 0, VW, Math.max(1, hz), 0xFF3F86DA, 0xFFCDE3F7);
        batch.fillRect(0, hz, VW, VH - hz + 2, 0xFFCDE3F7);
        float sunY = Math.min(hz * 0.45f, VH * 0.12f);
        batch.icon(Font.SOFT, VW * 0.84f, sunY, 300, 0x80FFF1B8);
        batch.icon(Font.CIRCLE, VW * 0.84f, sunY, 84, 0xFFFFF8E0);
        batch.flush();

        // fundo (Congresso), sem escrever profundidade
        g.state(false, false, false, true);
        g.colorPass(vp, 150f, 900f, SKY_R, SKY_G, SKY_B);
        Mat.identity(m0);
        Mat.translate(m0, 0f, -1.5f, -350f);
        g.drawMesh(models.backdrop, m0, 1, 1, 1, 1);

        g.colorPass(vp, FOG0, FOG1, SKY_R, SKY_G, SKY_B);
        int ca = Scenery.first(absP), cb = Scenery.last(absP);
        // chao
        for (int i = ca; i <= cb; i++) {
            Scenery.Chunk c = scenery.get(i);
            if (c.idx != i || c.ground == null) continue;
            Mat.identity(m0);
            Mat.translate(m0, 0f, 0f, (float) (absP - i * (double) Scenery.CH));
            g.drawMesh(c.ground, m0, 1, 1, 1, 1);
        }
        // objetos opacos
        g.state(true, true, false, true);
        for (int i = ca; i <= cb; i++) {
            Scenery.Chunk c = scenery.get(i);
            if (c.idx != i || c.props == null) continue;
            Mat.identity(m0);
            Mat.translate(m0, 0f, 0f, (float) (absP - i * (double) Scenery.CH));
            g.drawMesh(c.props, m0, 1, 1, 1, 1);
        }
        drawObstacles();
        drawCoins();
        drawPlayer();
        drawDuck();

        // transparentes 3D
        g.state(true, false, true, false);
        g.texPass(vp, tex, FOG0, FOG1, SKY_R, SKY_G, SKY_B);
        drawWorldSprites(absP, ca, cb);
        batch.flush();

        // interface
        g.state(false, false, true, false);
        g.texPass(ui, tex, 1, 0, 0, 0, 0);
        drawUI(dt);
        batch.flush();
    }

    private void drawObstacles() {
        World w = world;
        float P = w.P;
        for (World.Obs o : w.obs) {
            if (!o.alive) continue;
            if (o.g1 < P - 12f || o.g0 > P + 165f) continue;
            float z = P - o.g0;
            switch (o.type) {
                case World.O_LOW:
                    Mat.identity(m0);
                    Mat.translate(m0, o.x, 0, z);
                    g.drawMesh(models.low, m0, 1, 1, 1, 1);
                    break;
                case World.O_HIGH:
                    Mat.identity(m0);
                    Mat.translate(m0, o.x, 0, z);
                    g.drawMesh(models.high, m0, 1, 1, 1, 1);
                    break;
                case World.O_RAMP:
                    Mat.identity(m0);
                    Mat.translate(m0, o.x, 0, z);
                    g.drawMesh(models.ramp, m0, 1, 1, 1, 1);
                    break;
                default: {
                    for (int c = 0; c < o.cars; c++) {
                        float zc = z - c * World.CAR_LEN;
                        if (zc - World.CAR_LEN > 12f || zc < -170f) continue;
                        Mat.identity(m0);
                        Mat.translate(m0, o.x, 0, zc);
                        Gfx.Mesh m = o.moving ? (c == 0 ? models.train : models.trainCar) : models.bus[o.variant & 3];
                        g.drawMesh(m, m0, 1, 1, 1, 1);
                    }
                    break;
                }
            }
        }
    }

    private void drawCoins() {
        World w = world;
        float P = w.P;
        float spin = time * 3.2f;
        for (World.Coin c : w.coins) {
            if (!c.alive) continue;
            float dg = c.g - P;
            if (dg < -8f || dg > 120f) continue;
            Mat.identity(m0);
            Mat.translate(m0, c.x, c.y + (float) Math.sin(time * 4f + c.g) * 0.06f, -dg);
            Mat.rotY(m0, spin + c.g * 0.15f);
            Mat.scale(m0, 1.3f, 1.3f, 1.3f);
            g.drawMesh(models.coin, m0, 1, 1, 1, 1);
        }
    }

    private void drawPlayer() {
        World w = world;
        float x = w.px, y = w.py, yaw = lulaYaw;
        int si = skin;
        boolean jet = w.jet() && w.alive;
        boolean blink = w.invulnT > 0 && ((int) (time * 12) & 1) == 0 && state == PLAY;
        if (blink) return;

        float ph = w.runPhase;
        float legA, legB, armA, armB, sideA = 0.08f, sideB = 0.08f, lean = -0.12f, hipY = 0.86f, bob = 0f, headT = 0f;
        float roll = w.lean;
        if (state == MENU || state == SHOP) {
            float t = time;
            legA = 0.05f; legB = -0.05f;
            armA = 0f; armB = 0f;
            sideA = 0.1f;
            sideB = 2.5f + (float) Math.sin(t * 7f) * 0.35f; // acenando
            lean = 0.02f;
            bob = (float) Math.sin(t * 2.2f) * 0.01f;
            roll = 0;
        } else if (!w.alive) {
            float k = Math.min(1f, w.deathT * 3.5f);
            lean = 1.45f * k;
            hipY = 0.86f - 0.55f * k;
            legA = 1.3f * k; legB = 1.0f * k;
            armA = 2.7f * k; armB = 2.4f * k;
            headT = -0.3f * k;
            roll = 0;
        } else if (jet) {
            legA = 0.15f; legB = -0.15f;
            armA = 0f; armB = 0f;
            sideA = 1.25f + (float) Math.sin(time * 3f) * 0.1f;
            sideB = 1.25f - (float) Math.sin(time * 3f) * 0.1f;
            lean = 0.05f;
        } else if (w.sliding()) {
            lean = 1.0f;
            hipY = 0.4f;
            legA = 1.5f; legB = 1.3f;
            armA = 0.7f; armB = 0.5f;
            headT = -0.5f;
        } else if (!w.grounded) {
            legA = 0.75f; legB = -0.25f;
            armA = 2.6f; armB = 2.6f;
            sideA = 0.25f; sideB = 0.25f;
            lean = -0.05f;
        } else {
            float s = (float) Math.sin(ph);
            legA = 0.8f * s; legB = -0.8f * s;
            armA = -0.7f * s; armB = 0.7f * s;
            bob = Math.abs((float) Math.cos(ph)) * 0.06f;
        }

        // aviao
        if (jet || (w.powT[World.PW_JET] > 0 && !w.alive)) {
            Mat.identity(m0);
            Mat.translate(m0, x, y - 0.5f, 0f);
            Mat.rotZ(m0, roll * 0.6f + (float) Math.sin(time * 2f) * 0.05f);
            g.drawMesh(models.plane, m0, 1, 1, 1, 1);
            y += 0f;
        }

        Mat.identity(m0);
        Mat.translate(m0, x, y, 0f);
        Mat.rotY(m0, yaw);
        Mat.rotZ(m0, roll);
        Mat.translate(m0, 0f, hipY + bob, 0f);
        // pernas
        limb(models.leg[si], m0, -0.12f, 0f, 0f, legA, 0f);
        limb(models.leg[si], m0, 0.12f, 0f, 0f, legB, 0f);
        // tronco
        Mat.copy(m1, m0);
        Mat.rotX(m1, lean);
        g.drawMesh(models.torso[si], m1, 1, 1, 1, 1);
        if (si == 4 || w.powT[World.PW_MULT] > 0) g.drawMesh(models.sash, m1, 1, 1, 1, 1);
        limb(models.arm[si], m1, -0.37f, 0.58f, 0f, armA, -sideA);
        limb(models.arm[si], m1, 0.37f, 0.58f, 0f, armB, sideB);
        Mat.copy(m2, m1);
        Mat.translate(m2, 0f, 0.72f, 0f);
        Mat.rotX(m2, headT);
        g.drawMesh(models.head[si], m2, 1, 1, 1, 1);
        if (w.powT[World.PW_SHOES] > 0 && w.alive) {
            // chinelo turbo: brilho verde nos pes
            Mat.identity(m2);
            Mat.translate(m2, x, y + 0.04f, 0f);
            Mat.scale(m2, 0.62f, 0.05f, 0.5f);
            g.drawMesh(models.cube, m2, 0.2f, 0.9f, 0.35f, 1f);
        }
    }

    private void limb(Gfx.Mesh mesh, float[] parent, float x, float y, float z, float ax, float az) {
        Mat.copy(m2, parent);
        Mat.translate(m2, x, y, z);
        Mat.rotZ(m2, az);
        Mat.rotX(m2, ax);
        g.drawMesh(mesh, m2, 1, 1, 1, 1);
    }

    private void drawDuck() {
        World w = world;
        float dz, dx, yaw, sc = 0.72f;
        float phase = time * 9f;
        if (state == MENU || state == SHOP) {
            dz = -5.5f; dx = 2.6f; yaw = (float) Math.PI - 0.5f;
            sc = 0.9f;
            phase = time * 2f;
        } else {
            if (w.duckD > 25f) return;
            dz = w.duckD;
            dx = w.duckX + 0.75f;
            yaw = 0f;
            if (!w.alive && w.cause == World.CAUSE_CAUGHT) dz = Math.max(0.9f, dz);
        }
        boolean moving = state == PLAY || (state == OVER && w.deathT < 1.2f);
        float s = moving ? (float) Math.sin(phase) : 0f;
        Mat.identity(m0);
        Mat.translate(m0, dx, Math.abs(s) * 0.12f, dz);
        Mat.rotY(m0, yaw);
        Mat.rotZ(m0, s * 0.06f);
        Mat.scale(m0, sc, sc, sc);
        g.drawMesh(models.duckBody, m0, 1, 1, 1, 1);
        for (int k = -1; k <= 1; k += 2) {
            Mat.copy(m1, m0);
            Mat.translate(m1, k * 0.32f, 0.55f, -0.1f);
            Mat.rotX(m1, k * s * 0.6f);
            g.drawMesh(models.duckLeg, m1, 1, 1, 1, 1);
            Mat.copy(m1, m0);
            Mat.translate(m1, k * 0.8f, 1.25f, 0.1f);
            if (k < 0) Mat.rotY(m1, (float) Math.PI);
            Mat.rotZ(m1, 0.3f + Math.abs((float) Math.sin(phase * 1.3f)) * 0.5f);
            g.drawMesh(models.duckWing, m1, 1, 1, 1, 1);
        }
    }

    private void drawWorldSprites(double absP, int ca, int cb) {
        World w = world;
        float P = w.P;
        Batch b = batch;
        // sombras
        if (!(w.jet() && w.alive)) {
            float sy = w.surf + 0.03f;
            float sz = Math.max(0.3f, 0.55f - (w.py - w.surf) * 0.08f);
            b.ground3D(Font.SOFT, w.px, sy, 0f, sz, sz, 0x70000000);
        }
        if (state != MENU && state != SHOP && w.duckD < 25f) b.ground3D(Font.SOFT, w.duckX + 0.75f, 0.03f, w.duckD, 0.8f, 1.0f, 0x60000000);
        if (state == MENU || state == SHOP) b.ground3D(Font.SOFT, 2.6f, 0.03f, -5.5f, 1.0f, 1.2f, 0x60000000);

        // outdoors e placas do cenario
        for (int i = ca; i <= cb; i++) {
            Scenery.Chunk c = scenery.get(i);
            if (c.idx != i) continue;
            float zb = (float) (absP - i * (double) Scenery.CH);
            for (int t = 0; t < c.nt; t++) {
                float z = zb - c.ts[t];
                if (z < -150f || z > 8f) continue;
                b.sign3D(c.tstr[t], c.tx[t], c.ty[t], z, c.tw[t], c.th[t], c.tcol[t]);
            }
        }
        // placas dos obstaculos
        for (World.Obs o : w.obs) {
            if (!o.alive) continue;
            float z = P - o.g0;
            if (z < -110f || z > 6f) continue;
            switch (o.type) {
                case World.O_LOW:
                    b.sign3D(Memes.LOW_SIGNS[o.variant % Memes.LOW_SIGNS.length], o.x, 0.725f, z + 0.03f, 1.0f, 0.28f, 0xFF111111);
                    break;
                case World.O_HIGH: {
                    int col = (o.variant & 1) == 0 ? 0xFFB91C1C : 0xFF1F2937;
                    float y0 = World.HIGH_BOT + 0.11f, y1 = 2.85f;
                    char wc = Font.WHITE;
                    b.quad(o.x - 1.02f, y1, z + 0.005f, o.x + 1.02f, y1, z + 0.005f, o.x + 1.02f, y0, z + 0.005f, o.x - 1.02f, y0, z + 0.005f,
                            font.u0[wc], font.v0[wc], font.u1[wc], font.v1[wc], col);
                    b.sign3D(Memes.BANNERS[(o.variant >> 1) % Memes.BANNERS.length], o.x, (y0 + y1) * 0.5f, z + 0.02f, 1.85f, 1.2f, 0xFFFFFFFF);
                    break;
                }
                case World.O_BUS:
                    if (o.moving) {
                        b.sign3D("TREM-BALA", o.x, 0.97f, z + 0.04f, 1.1f, 0.24f, 0xFFFFD60A);
                        b.sprite3D(Font.SOFT, o.x - 0.75f, 0.6f, z + 0.2f, 1.2f, 0xB0FFF3B0);
                        b.sprite3D(Font.SOFT, o.x + 0.75f, 0.6f, z + 0.2f, 1.2f, 0xB0FFF3B0);
                    } else {
                        b.sign3D(Memes.BUS_DEST[(o.variant >> 2) % Memes.BUS_DEST.length], o.x, 2.325f, z + 0.04f, 1.5f, 0.22f, 0xFFFFB020);
                    }
                    break;
                default:
                    break;
            }
        }
        // power-ups
        for (World.Pow p : w.pows) {
            if (!p.alive) continue;
            float z = P - p.g;
            if (z < -120f || z > 6f) continue;
            float yy = p.y + (float) Math.sin(time * 4f) * 0.15f;
            int col = POW_COL[p.kind];
            b.sprite3D(Font.SOFT, p.x, yy, z, 2.2f, (col & 0xFFFFFF) | 0xA0000000);
            b.sprite3D(Font.CIRCLE, p.x, yy, z + 0.01f, 1.15f, 0xFFFFFFFF);
            b.sprite3D(POW_ICON[p.kind], p.x, yy, z + 0.02f, 0.95f, 0xFFFFFFFF);
        }
        // brilho das picanhas
        for (World.Coin c : w.coins) {
            if (!c.alive) continue;
            float dg = c.g - P;
            if (dg < -6f || dg > 110f) continue;
            b.sprite3D(Font.SOFT, c.x, c.y, -dg - 0.3f, 1.5f, 0x55FFD34D);
        }
        // particulas
        for (int i = 0; i < NP; i++) {
            if (ptT[i] <= 0) continue;
            float a = ptT[i] / 0.45f;
            int col = ((int) (a * 255) << 24) | (ptC[i] & 0xFFFFFF);
            b.sprite3D(Font.SOFT, ptX[i], ptY[i], ptZ[i], 0.5f + (1 - a) * 0.6f, col);
        }
    }

    static final int[] POW_COL = {0xE53935, 0x3B82F6, 0x16A34A, 0xF2B705};
    static final char[] POW_ICON = {Font.MAGNET, Font.PLANE, Font.SHOE, Font.MULT};

    // ================================================================ interface
    private boolean button(String label, float x, float y, float w, float h, int col, float size) {
        boolean hit = tapPending && tapX >= x && tapX <= x + w && tapY >= y && tapY <= y + h;
        batch.round(x, y + 8, w, h, h * 0.3f, 0x55000000);
        batch.round(x, y, w, h, h * 0.3f, 0xFF000000 | col);
        batch.round(x + 6, y + 6, w - 12, h * 0.45f, h * 0.25f, 0x26FFFFFF);
        batch.textFit(label, x + w * 0.5f, y + h * 0.52f, size, w - 30, 0xFFFFFFFF, Batch.CENTER);
        if (hit) {
            tapPending = false;
            sound.play(Synth.CLICK, 0.6f, 1f);
        }
        return hit;
    }

    private boolean iconButton(char icon, float cxp, float cyp, float r) {
        boolean hit = tapPending && Math.abs(tapX - cxp) <= r * 1.2f && Math.abs(tapY - cyp) <= r * 1.2f;
        batch.icon(Font.CIRCLE, cxp, cyp + 4, r * 2, 0x55000000);
        batch.icon(Font.CIRCLE, cxp, cyp, r * 2, 0xCC1F2937);
        batch.icon(icon, cxp, cyp, r * 1.25f, 0xFFFFFFFF);
        if (hit) {
            tapPending = false;
            sound.play(Synth.CLICK, 0.6f, 1f);
        }
        return hit;
    }

    private CharSequence num(int v) {
        sb.setLength(0);
        appendInt(sb, v);
        return sb;
    }

    private static void appendInt(StringBuilder s, int v) {
        if (v < 0) { s.append('-'); v = -v; }
        int start = s.length();
        do {
            s.append((char) ('0' + v % 10));
            v /= 10;
        } while (v > 0);
        // inverte
        for (int i = start, j = s.length() - 1; i < j; i++, j--) {
            char t = s.charAt(i);
            s.setCharAt(i, s.charAt(j));
            s.setCharAt(j, t);
        }
    }

    private void picanhaBox(float x, float y, int value) {
        sb.setLength(0);
        appendInt(sb, value);
        float tw = batch.textWidth(sb, 46);
        batch.round(x, y, tw + 96, 64, 22, 0x99000000);
        batch.icon(Font.PICANHA, x + 38, y + 32, 58, 0xFFFFFFFF);
        batch.text(sb, x + 72, y + 33, 46, 0xFFFFE08A, Batch.LEFT);
    }

    private void drawUI(float dt) {
        switch (state) {
            case MENU: drawMenu(); break;
            case SHOP: drawShop(); break;
            case PLAY: drawHud(); break;
            case COUNT: drawHud(); drawCount(); break;
            case PAUSE: drawHud(); drawPause(); break;
            case OVER: drawHud(); drawOver(); break;
            default: break;
        }
        drawPopups();
    }

    private void drawMenu() {
        Batch b = batch;
        float t = time;
        b.gradient(0, 0, VW, VH * 0.36f, 0x99000000, 0x00000000);
        b.text("CORRE,", VW * 0.5f, VH * 0.075f + 6, 84, 0xFFFFFFFF, Batch.CENTER);
        float s = 1f + (float) Math.sin(t * 3f) * 0.03f;
        b.text("LULA!", VW * 0.5f + 5, VH * 0.16f + 7, 170 * s, 0xFF0B6B2E, Batch.CENTER);
        b.text("LULA!", VW * 0.5f, VH * 0.16f, 170 * s, 0xFFFFD60A, Batch.CENTER);
        if (((int) (t / 3.5f)) % Memes.MENU_SUB.length != menuSub) menuSub = ((int) (t / 3.5f)) % Memes.MENU_SUB.length;
        b.textFit(Memes.MENU_SUB[menuSub], VW * 0.5f, VH * 0.245f, 44, VW - 60, 0xFFFFFFFF, Batch.CENTER);

        picanhaBox(24, 24 + VH * 0.27f, bank);
        sb.setLength(0);
        sb.append("RECORDE ");
        appendInt(sb, best);
        float tw = b.textWidth(sb, 38);
        b.round(VW - tw - 70, 24 + VH * 0.27f, tw + 46, 64, 22, 0x99000000);
        b.icon(Font.STAR, VW - tw - 46, 56 + VH * 0.27f, 44, 0xFFFFFFFF);
        b.text(sb, VW - 24 - 12, 57 + VH * 0.27f, 38, 0xFFFFFFFF, Batch.RIGHT);

        float by = VH - 330;
        if (button("JOGAR", VW * 0.5f - 230, by, 460, 130, 0x16A34A, 84)) { startRun(); return; }
        if (button("LOJA", VW * 0.5f - 230, by + 160, 300, 96, 0xD97706, 56)) { setState(SHOP); return; }
        if (iconButton(muted ? Font.MUTE : Font.SOUND, VW * 0.5f + 182, by + 208, 46)) toggleMute();
        b.textFit("Paródia não oficial, sem fins lucrativos. Desenhos e sons gerados por código.",
                VW * 0.5f, VH - 34, 22, VW - 40, 0xCCFFFFFF, Batch.CENTER);
        if (tutorialPending) {
            b.textFit("Deslize o dedo para jogar!", VW * 0.5f, by - 44, 34, VW - 60, 0xFFFFFFFF, Batch.CENTER);
        }
    }

    private void toggleMute() {
        muted = !muted;
        store.put("mute", muted ? 1 : 0);
        sound.setMuted(muted);
    }

    private void drawShop() {
        Batch b = batch;
        // layout que se adapta a altura da tela
        float need = 1030f;
        float k = Math.min(1f, (VH - 60f) / need);
        float rowP = 86f * k, rowS = 72f * k;
        float top = Math.max(150f * k, VH - need * k - 20f) + 100f * k;
        b.fillRect(0, 0, VW, VH, 0x40000000);
        b.round(20, top - 120 * k, VW - 40, VH - top + 100 * k, 40, 0xF0FFFFFF);
        b.text("LOJA DA COMPANHEIRADA", VW * 0.5f, top - 80 * k, 46 * Math.max(k, 0.85f), 0xFF0B6B2E, Batch.CENTER);
        picanhaBox(VW * 0.5f - 110, top - 44 * k, bank);
        float y = top + 56 * k;
        b.text("PODERES", 50, y, 34, 0xFF1F2937, Batch.LEFT);
        y += 30 * k;
        String[] names = Memes.POW_NAME;
        for (int i = 0; i < 4; i++) {
            float ry = y + i * rowP;
            float rh = rowP - 8;
            b.round(40, ry, VW - 80, rh, 22, 0x14000000);
            b.icon(POW_ICON[i], 88, ry + rh * 0.5f, rh * 0.75f, 0xFFFFFFFF);
            b.textFit(i == 3 ? "FAZ O L (2X)" : names[i], 130, ry + rh * 0.32f, 32, 300, 0xFF111827, Batch.LEFT);
            for (int q = 0; q < 5; q++) {
                b.round(130 + q * 40, ry + rh * 0.62f, 32, 16, 8, q < lv[i] ? 0xFF16A34A : 0x33000000);
            }
            if (lv[i] >= 5) {
                b.text("MÁX", VW - 120, ry + rh * 0.5f, 36, 0xFF16A34A, Batch.CENTER);
            } else {
                int price = 150 * (lv[i] + 1);
                sb.setLength(0);
                sb.append(Font.PICANHA);
                appendInt(sb, price);
                if (button(sb.toString(), VW - 230, ry + rh * 0.12f, 180, rh * 0.76f, bank >= price ? 0xD97706 : 0x9CA3AF, 36)) {
                    if (bank >= price) {
                        bank -= price;
                        lv[i]++;
                        store.put("bank", bank);
                        store.put("lv" + i, lv[i]);
                        sound.play(Synth.BUY, 0.7f, 1f);
                        enterMenuKeepState();
                    }
                }
            }
        }
        y += 4 * rowP + 26 * k;
        b.text("VISUAL DO LULA", 50, y, 34, 0xFF1F2937, Batch.LEFT);
        y += 30 * k;
        int ns = Memes.SKIN_NAME.length;
        float rowH = rowS;
        for (int i = 0; i < ns; i++) {
            float ry = y + i * rowH;
            boolean owned = (skinsOwned & (1 << i)) != 0;
            b.round(40, ry, VW - 80, rowH - 8, 20, i == skin ? 0x3316A34A : 0x14000000);
            b.textFit(Memes.SKIN_NAME[i], 64, ry + rowH * 0.45f, 32, 360, 0xFF111827, Batch.LEFT);
            String lab;
            int col;
            if (i == skin) { lab = "USANDO"; col = 0x16A34A; }
            else if (owned) { lab = "USAR"; col = 0x2563EB; }
            else { lab = Font.PICANHA + "" + Memes.SKIN_PRICE[i]; col = bank >= Memes.SKIN_PRICE[i] ? 0xD97706 : 0x9CA3AF; }
            if (button(lab, VW - 230, ry + 6, 180, rowH - 20, col, 32) && i != skin) {
                if (owned) {
                    skin = i;
                    store.put("skin", skin);
                } else if (bank >= Memes.SKIN_PRICE[i]) {
                    bank -= Memes.SKIN_PRICE[i];
                    skinsOwned |= 1 << i;
                    skin = i;
                    store.put("bank", bank);
                    store.put("skins", skinsOwned);
                    store.put("skin", skin);
                    sound.play(Synth.BUY, 0.7f, 1f);
                }
            }
        }
        if (button("VOLTAR", VW * 0.5f - 150, VH - 30 - 96 * k, 300, 88 * k, 0x4B5563, 50)) setState(MENU);
    }

    private void enterMenuKeepState() {
        world.powDur[0] = 10f + lv[0] * 1.5f;
        world.powDur[1] = 8f + lv[1] * 1.5f;
        world.powDur[2] = 10f + lv[2] * 1.5f;
        world.powDur[3] = 12f + lv[3] * 1.5f;
    }

    private void drawHud() {
        Batch b = batch;
        World w = world;
        if (state == PLAY && iconButton(Font.PAUSE, 64, 70, 40)) {
            setState(PAUSE);
        }
        picanhaBox(120, 38, w.coinsRun);
        // pontos
        CharSequence sc = num((int) w.score);
        b.text(sc, VW - 26, 72, 72, 0xFFFFFFFF, Batch.RIGHT);
        if (w.powT[World.PW_MULT] > 0) {
            b.icon(Font.MULT, VW - 60, 140, 62, 0xFFFFFFFF);
        }
        // timers de poder
        float y = 140;
        for (int i = 0; i < 4; i++) {
            float t = w.powT[i];
            if (t <= 0 || state == OVER) continue;
            float frac = t / w.powDur[i];
            b.round(24, y, 230, 54, 18, 0x99000000);
            b.icon(POW_ICON[i], 52, y + 27, 46, 0xFFFFFFFF);
            b.round(84, y + 19, 156, 16, 8, 0x55FFFFFF);
            boolean warn = t < 2f && ((int) (time * 8) & 1) == 0;
            b.round(84, y + 19, Math.max(16, 156 * frac), 16, 8, warn ? 0xFFFFFFFF : (0xFF000000 | POW_COL[i]));
            y += 62;
        }
        // tutorial
        if (w.tutorial && w.dist < 330f && state == PLAY && !w.jet()) {
            String hint = hint();
            if (hint != null) {
                float pulse = 1f + (float) Math.sin(time * 8f) * 0.04f;
                b.round(40, VH * 0.62f - 50, VW - 80, 100, 30, 0xAA000000);
                b.textFit(hint, VW * 0.5f, VH * 0.62f, 52 * pulse, VW - 120, 0xFFFFE08A, Batch.CENTER);
            }
        }
        if (introT > 0.2f && state == PLAY) {
            b.textFit("CORRE, COMPANHEIRO!", VW * 0.5f, VH * 0.36f, 64, VW - 40, 0xFFFFD60A, Batch.CENTER);
        }
        // fala do Lula
        if (sayT > 0 && sayText != null && w.alive && project(w.px, w.py + 2.35f, 0f)) {
            float a = Math.min(1f, sayT * 3f) * Math.min(1f, (2.4f - sayT) * 6f);
            int lines = Batch.lineCount(sayText);
            float maxw = b.linesWidth(sayText, 36);
            float bw = Math.min(VW - 40, maxw + 50), bh = lines * 36 + 34;
            float bx = Math.max(20, Math.min(VW - 20 - bw, pj[0] - bw * 0.5f));
            float byy = Math.max(170, pj[1] - bh - 40);
            int al = (int) (a * 255) << 24;
            float tx = Math.max(bx + 30, Math.min(bx + bw - 30, pj[0]));
            char wc = Font.WHITE;
            b.quad(tx - 18, byy + bh - 2, 0, tx + 18, byy + bh - 2, 0, tx + 4, byy + bh + 30, 0, tx, byy + bh + 30, 0,
                    font.u0[wc], font.v0[wc], font.u1[wc], font.v1[wc], al | 0xFFFFFF);
            b.round(bx, byy, bw, bh, 24, al | 0xFFFFFF);
            b.textLines(sayText, bx + bw * 0.5f, byy + bh * 0.5f, 36, bw - 30, al | 0x111111);
        }
    }

    private String hint() {
        World w = world;
        World.Obs best = null;
        float bd = 1e9f;
        for (World.Obs o : w.obs) {
            if (!o.alive) continue;
            float d = o.g0 - w.P;
            if (d < 3f || d > 30f) continue;
            if (Math.abs(o.x - w.lane * World.LANE_W) > 0.5f) continue;
            if (d < bd) { bd = d; best = o; }
        }
        if (best == null) return null;
        switch (best.type) {
            case World.O_LOW: return "DESLIZE " + Font.UP + " PARA PULAR";
            case World.O_HIGH: return "DESLIZE " + Font.DOWN + " PARA ABAIXAR";
            case World.O_RAMP: return "SUBA NA RAMPA!";
            default: return "DESLIZE " + Font.LEFT + " " + Font.RIGHT + " PARA DESVIAR";
        }
    }

    private void drawCount() {
        int n = 3 - (int) stateT;
        if (n < 1) n = 1;
        float f = stateT - (int) stateT;
        batch.text(num(n), VW * 0.5f, VH * 0.42f, 220 * (1.3f - f * 0.3f), 0xFFFFD60A, Batch.CENTER);
    }

    private void drawPause() {
        Batch b = batch;
        b.fillRect(0, 0, VW, VH, 0x88000000);
        b.text("PAUSADO", VW * 0.5f, VH * 0.3f, 110, 0xFFFFD60A, Batch.CENTER);
        b.textFit("Companheiro foi tomar um cafezinho", VW * 0.5f, VH * 0.3f + 90, 36, VW - 60, 0xFFFFFFFF, Batch.CENTER);
        if (button("CONTINUAR", VW * 0.5f - 220, VH * 0.48f, 440, 120, 0x16A34A, 64)) { resume(); return; }
        if (button("MENU", VW * 0.5f - 220, VH * 0.48f + 150, 440, 100, 0x4B5563, 56)) {
            commitRun();
            finishOver(true);
            return;
        }
        if (iconButton(muted ? Font.MUTE : Font.SOUND, VW * 0.5f, VH * 0.48f + 330, 50)) toggleMute();
    }

    private void drawOver() {
        Batch b = batch;
        World w = world;
        if (overDelay > 0) return;
        float a = Math.min(1f, stateT * 2f);
        b.fillRect(0, 0, VW, VH, ((int) (a * 120) << 24));
        b.textLines(overTitle, VW * 0.5f, VH * 0.15f, 84, VW - 40, 0xFFFFD60A);
        float px0 = 50, py0 = VH * 0.27f, pw = VW - 100, ph = 470;
        b.round(px0, py0 + 10, pw, ph, 40, 0x55000000);
        b.round(px0, py0, pw, ph, 40, 0xF2FFFFFF);
        b.text("PONTOS", VW * 0.5f, py0 + 50, 38, 0xFF6B7280, Batch.CENTER);
        b.text(num((int) w.score), VW * 0.5f, py0 + 128, 110, 0xFF111827, Batch.CENTER);
        if (newRecord) {
            float pulse = 1f + (float) Math.sin(time * 6f) * 0.04f;
            b.round(px0 + 20, py0 + 196, pw - 40, 110, 26, 0xFFFFD60A);
            b.textLines(Memes.RECORD, VW * 0.5f, py0 + 251, 40 * pulse, pw - 80, 0xFF7A4B00);
        } else {
            sb.setLength(0);
            sb.append("RECORDE: ");
            appendInt(sb, best);
            b.text(sb, VW * 0.5f, py0 + 220, 40, 0xFF6B7280, Batch.CENTER);
            sb.setLength(0);
            appendInt(sb, (int) w.dist);
            sb.append(" METROS");
            b.text(sb, VW * 0.5f, py0 + 275, 36, 0xFF6B7280, Batch.CENTER);
        }
        sb.setLength(0);
        sb.append('+');
        appendInt(sb, w.coinsRun);
        sb.append(" PICANHAS");
        b.icon(Font.PICANHA, VW * 0.5f - b.textWidth(sb, 44) * 0.5f - 40, py0 + 360, 64, 0xFFFFFFFF);
        b.text(sb, VW * 0.5f + 10, py0 + 360, 44, 0xFFB45309, Batch.CENTER);
        sb.setLength(0);
        sb.append("Total: ");
        appendInt(sb, bank);
        b.text(sb, VW * 0.5f, py0 + 420, 32, 0xFF6B7280, Batch.CENTER);

        float by = py0 + ph + 40;
        int cost = 50 << w.revives;
        if (w.revives < 3 && bank >= cost && reviveT > 0) {
            sb.setLength(0);
            sb.append("CONTINUAR ");
            sb.append(Font.PICANHA);
            appendInt(sb, cost);
            if (button(sb.toString(), VW * 0.5f - 250, by, 500, 110, 0x2563EB, 52)) {
                bank -= cost;
                store.put("bank", bank);
                w.revive();
                banked = w.coinsRun;
                setState(COUNT);
                say("Não vou pagar\no pato!");
                return;
            }
            b.round(VW * 0.5f - 230, by + 116, 460 * (reviveT / 6f), 10, 5, 0xFF93C5FD);
            by += 140;
        }
        if (button("DE NOVO", VW * 0.5f - 250, by, 500, 110, 0x16A34A, 60)) { restart(); return; }
        if (button("MENU", VW * 0.5f - 250, by + 130, 500, 90, 0x4B5563, 50)) { finishOver(true); }
    }

    private void drawPopups() {
        for (int i = 0; i < 3; i++) {
            if (popT[i] <= 0 || popText[i] == null) continue;
            float t = 1.6f - popT[i];
            float sc = t < 0.15f ? 0.6f + t / 0.15f * 0.55f : (t < 0.3f ? 1.15f - (t - 0.15f) : 1f);
            float a = Math.min(1f, popT[i] * 2.5f);
            float y = VH * 0.30f + i * 80 - t * 40;
            int col = ((int) (a * 255) << 24) | (popCol[i] & 0xFFFFFF);
            batch.textFit(popText[i], VW * 0.5f, y, 74 * sc, VW - 40, col, Batch.CENTER);
        }
    }
}
