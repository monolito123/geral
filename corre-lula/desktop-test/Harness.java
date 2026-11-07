package com.monolito.correlula;

import java.util.HashMap;

/**
 * Testes no desktop:
 *   shots  -> gera capturas de tela (menu, corrida, power-ups, derrota, loja)
 *   bot    -> simula muitas corridas com um bot para checar se o gerador e justo
 *   audio  -> exporta os sons para WAV
 */
public final class Harness {
    static final class MemStore implements Game.Store {
        final HashMap<String, Integer> m = new HashMap<>();
        public int get(String k, int d) { Integer v = m.get(k); return v == null ? d : v; }
        public void put(String k, int v) { m.put(k, v); }
    }

    static final class NoSound implements Game.Sound {
        int plays;
        public void play(int id, float vol, float rate) { plays++; }
        public void setMuted(boolean m) {}
        public void setMusic(boolean on) {}
    }

    public static void main(String[] a) throws Exception {
        String mode = a.length > 0 ? a[0] : "shots";
        String out = a.length > 1 ? a[1] : ".";
        if (mode.equals("shots")) shots(out, a.length > 2 ? Integer.parseInt(a[2]) : 540, a.length > 3 ? Integer.parseInt(a[3]) : 1170);
        else if (mode.equals("bot")) bot(a.length > 1 ? Integer.parseInt(a[1]) : 20, a.length > 2 ? Float.parseFloat(a[2]) : 300f);
        else if (mode.equals("smart")) { smart = true; bot(a.length > 1 ? Integer.parseInt(a[1]) : 20, a.length > 2 ? Float.parseFloat(a[2]) : 300f); }
        else if (mode.equals("audio")) audio(out);
        else if (mode.equals("debug")) { if (a.length > 3) dbgT = Float.parseFloat(a[3]); debugRun(Long.parseLong(a[1]), Float.parseFloat(a[2])); }
        else if (mode.equals("cam")) cam(out, a);
    }

    // ------------------------------------------------------------------ capturas
    static Game game;
    static SoftGfx gfx;
    static String outDir;
    static int W, H;

    static void tap(float nx, float ny) {
        game.input(Game.T_DOWN, nx, ny);
        game.input(Game.T_UP, nx, ny);
    }

    static void frames(int n, boolean botOn) {
        for (int i = 0; i < n; i++) {
            if (botOn) botStep(game.world);
            game.frame(1f / 30f);
        }
    }

    static void shot(String name) throws Exception {
        long t0 = System.nanoTime();
        game.frame(1f / 60f);
        gfx.save(outDir + "/" + name + ".png");
        System.out.printf("%s.png  (%.0f ms, %d tris)%n", name, (System.nanoTime() - t0) / 1e6, gfx.tris);
        gfx.tris = 0;
    }

    static void shots(String out, int w, int h) throws Exception {
        outDir = out;
        W = w; H = h;
        MemStore st = new MemStore();
        st.put("bank", 1234);
        st.put("best", 4321);
        game = new Game(st, new NoSound());
        gfx = new SoftGfx(w, h);
        game.surfaceCreated(gfx, new AwtPainter());
        game.surfaceChanged(w, h);
        // atlas
        java.awt.image.BufferedImage img = new java.awt.image.BufferedImage(game.font.aw, game.font.ah, java.awt.image.BufferedImage.TYPE_INT_ARGB);
        img.setRGB(0, 0, game.font.aw, game.font.ah, game.font.pixels, 0, game.font.aw);
        javax.imageio.ImageIO.write(img, "png", new java.io.File(out + "/atlas.png"));

        frames(20, false);
        shot("01_menu");
        // loja
        tap(0.5f - 40f / 720f, (game.VH - 330 + 160 + 48) / game.VH);
        frames(25, false);
        shot("02_loja");
        game.input(Game.T_BACK, 0, 0);
        frames(25, false);
        // jogar
        tap(0.5f, (game.VH - 330 + 65) / game.VH);
        frames(8, true);
        shot("03_inicio");
        frames(60, true);
        shot("04_corrida");
        frames(90, true);
        shot("05_corrida2");
        game.world.activate(World.PW_JET);
        frames(45, true);
        shot("06_aerolula");
        frames(300, true);
        game.world.activate(World.PW_MULT);
        game.world.activate(World.PW_MAGNET);
        frames(20, true);
        shot("07_poderes");
        // abaixa
        game.input(Game.T_KEY, World.SW_DOWN, 0);
        frames(3, false);
        shot("08_deslize");
        // tropeco: forca o pato
        game.world.stumbleT = 3f;
        frames(30, true);
        shot("09_pato");
        // derrota: anda sem bot ate bater
        int guard = 0;
        while (game.world.alive && guard++ < 3000) frames(1, false);
        frames(70, false);
        shot("10_derrota");
        // pausa
        tap(0.5f, (game.VH * 0.27f + 470 + 40 + 140 + 55) / game.VH);
        frames(5, false);
        frames(100, true);
        game.input(Game.T_BACK, 0, 0);
        frames(3, false);
        shot("11_pausa");
    }

    static void cam(String out, String[] a) throws Exception {
        Game.CAM_Y = Float.parseFloat(a[2]); Game.CAM_Z = Float.parseFloat(a[3]);
        Game.CAM_TY = Float.parseFloat(a[4]); Game.CAM_TZ = Float.parseFloat(a[5]); Game.HFOV = Float.parseFloat(a[6]);
        outDir = out;
        MemStore st = new MemStore();
        st.put("tut", 1);
        game = new Game(st, new NoSound());
        gfx = new SoftGfx(540, 1170);
        game.surfaceCreated(gfx, new AwtPainter());
        game.surfaceChanged(540, 1170);
        tap(0.5f, (game.VH - 330 + 65) / game.VH);
        frames(150, true);
        shot("cam_" + a[7]);
    }

    // ------------------------------------------------------------------ bot
    static float laneClear(World w, int lane, float horizon) {
        float best = horizon;
        float x = lane * World.LANE_W;
        for (World.Obs o : w.obs) {
            if (!o.alive || o.type != World.O_BUS) continue;
            if (Math.abs(o.x - x) > 0.5f) continue;
            if (o.g1 < w.P - 0.5f) continue;
            float d = o.g0 - w.P;
            // ônibus com rampa na frente conta como livre
            boolean ramp = false;
            for (World.Obs r : w.obs) {
                if (r.alive && r.type == World.O_RAMP && Math.abs(r.x - x) < 0.5f && Math.abs(r.g1 - o.g0) < 0.2f && r.g0 > w.P - 1f) ramp = true;
            }
            if (ramp && !o.moving) continue;
            if (o.moving && o.act) d = d * w.speed / (w.speed + o.ms);
            else if (o.moving) d = Math.max(0, d - World.ACT) * 1f + Math.min(d, World.ACT) * w.speed / (w.speed + o.ms);
            if (d < 0) d = 0;
            // ja estou em cima dele?
            if (w.py >= World.BUS_H - 0.1f && Math.abs(w.px - x) < 0.5f && o.g0 <= w.P && o.g1 >= w.P) continue;
            if (d < best) best = d;
        }
        return best;
    }

    static boolean sideBlocked(World w, int lane) {
        float x = lane * World.LANE_W;
        for (World.Obs o : w.obs) {
            if (!o.alive || (o.type != World.O_BUS && o.type != World.O_RAMP)) continue;
            if (Math.abs(o.x - x) > 0.5f) continue;
            float top = o.type == World.O_BUS ? World.BUS_H : World.BUS_H * World.clamp((w.P + 3 - o.g0) / (o.g1 - o.g0), 0, 1);
            if (w.py >= top - 0.4f) continue;
            float lead = 3f + w.speed * 0.12f;
            if (o.g0 < w.P + lead && o.g1 > w.P - 1.5f) return true;
        }
        return false;
    }

    static float decideT;

    static void botStep(World w) {
        if (!w.alive) return;
        if (Math.abs(w.px - w.lane * World.LANE_W) > 0.05f) return; // trocando de faixa
        float hz = 60f;
        int cur = w.lane;
        float cc = laneClear(w, cur, hz);
        float need = 6f + w.speed * 0.55f;
        if (cc < need) {
            int bestL = cur;
            float bestC = cc;
            for (int d = -1; d <= 1; d += 2) {
                int l = cur + d;
                if (l < -1 || l > 1) continue;
                if (sideBlocked(w, l)) continue;
                float c = laneClear(w, l, hz);
                if (c > bestC + 0.5f) { bestC = c; bestL = l; }
                // duas faixas
                int l2 = l + d;
                if (l2 >= -1 && l2 <= 1 && !sideBlocked(w, l2)) {
                    float c2 = laneClear(w, l2, hz);
                    if (c2 > bestC + 0.5f && c2 > need) { bestC = c2; bestL = l; }
                }
            }
            if (bestL != cur) {
                w.swipe(bestL < cur ? World.SW_LEFT : World.SW_RIGHT);
                return;
            }
        }
        // barreiras na faixa atual
        float x = cur * World.LANE_W;
        for (World.Obs o : w.obs) {
            if (!o.alive || Math.abs(o.x - x) > 0.5f) continue;
            float d = o.g0 - w.P;
            if (d < 0.2f) continue;
            float t = d / w.speed;
            if (o.type == World.O_LOW && w.grounded && w.py < 0.5f && t < 0.2f) { w.swipe(World.SW_UP); return; }
            if (o.type == World.O_HIGH && t < 0.16f && !w.sliding()) { w.swipe(World.SW_DOWN); return; }
        }
    }

    // bot com previsao: testa sequencias de 2 acoes e escolhe a que sobrevive
    static final World simW = new World();
    static { simW.sim = true; }
    static final int[] ACTS = {-1, World.SW_LEFT, World.SW_RIGHT, World.SW_UP, World.SW_DOWN};
    static int botTick;

    static float rollout(World w, int a1, int a2) {
        simW.copyFrom(w);
        int stumbles = 0;
        float st0 = simW.stumbleT;
        if (a1 >= 0) simW.swipe(a1);
        float t = 0, dt = 1f / 60f;
        boolean second = false;
        while (t < 1.7f && simW.alive) {
            if (!second && t >= 0.28f) { second = true; if (a2 >= 0) simW.swipe(a2); }
            float before = simW.stumbleT;
            simW.update(dt);
            if (simW.stumbleT > before + 0.5f) stumbles++;
            t += dt;
        }
        float score = t;
        if (!simW.alive) score = t * 10f - 1000f;
        score -= stumbles * 200f;
        if (a1 >= 0) score -= 0.3f;
        if (a2 >= 0) score -= 0.1f;
        // prefere ficar no chao/nas faixas do meio
        score -= Math.abs(simW.lane) * 0.05f;
        return score;
    }

    static void botSmart(World w) {
        if (!w.alive) return;
        if ((botTick++ % 4) != 0) return;
        float best = -1e9f;
        int bestA = -1;
        for (int a1 : ACTS) {
            for (int a2 : ACTS) {
                float sc = rollout(w, a1, a2);
                if (sc > best) { best = sc; bestA = a1; }
            }
        }
        if (bestA >= 0) w.swipe(bestA);
    }

    static boolean smart;
    static float dbgT = -1;

    static void debugRun(long seed, float seconds) {
        World w = new World();
        w.reset(seed, seed == 1000);
        botTick = 0;
        w.listener = new World.Listener() { public void onEvent(int ev, int arg) {} };
        w.pregen();
        w.started = true;
        float t = 0;
        java.util.Random pr = new java.util.Random(seed - 1000);
        int N = 600;
        float[][] hist = new float[N][];
        int hn = 0;
        java.util.ArrayList<String> snaps = new java.util.ArrayList<>();
        while (w.alive && t < seconds) {
            if (dbgT > 0 && t > dbgT && t < dbgT + 0.35f && (botTick % 4) == 0) {
                StringBuilder b = new StringBuilder(String.format("t=%.2f lane=%d slide=%.2f:", t, w.lane, w.slideT));
                for (int a1 : ACTS) for (int a2 : ACTS) b.append(String.format(" %d/%d=%.1f", a1, a2, rollout(w, a1, a2)));
                System.out.println(b);
                rollout(w, 0, 0);
                World.Obs o = simW.lastHit;
                System.out.printf("   L,L -> alive=%b lane=%d px=%.2f py=%.2f hit=%s%n", simW.alive, simW.lane, simW.px, simW.py,
                        o == null ? "-" : ("tipo " + o.type + " lane " + o.lane + " g " + (o.g0 - simW.P) + ".." + (o.g1 - simW.P)));
            }
            botSmart(w);
            if (pr.nextFloat() < 0.0015f) w.activate(pr.nextInt(4));
            hist[hn % N] = new float[]{t, w.P, w.lane, w.px, w.py, w.speed};
            if (hn % 15 == 0) {
                StringBuilder b = new StringBuilder();
                b.append(String.format("t=%.2f lane=%d px=%.2f py=%.2f |", t, w.lane, w.px, w.py));
                for (World.Obs o : w.obs) {
                    if (!o.alive) continue;
                    float d0 = o.g0 - w.P, d1 = o.g1 - w.P;
                    if (d1 < -2 || d0 > 60) continue;
                    b.append(String.format(" [%s L%d %.0f..%.0f%s]", "LHBR".charAt(o.type), o.lane, d0, d1, o.moving ? "m" : ""));
                }
                snaps.add(b.toString());
            }
            hn++;
            w.update(1f / 60f);
            t += 1f / 60f;
        }
        System.out.println("alive=" + w.alive + " t=" + t);
        for (int i = Math.max(0, snaps.size() - 14); i < snaps.size(); i++) System.out.println(snaps.get(i));
    }

    static void bot(int runs, float seconds) {
        int crash = 0, caught = 0, survived = 0;
        double totalDist = 0;
        for (int r = 0; r < runs; r++) {
            World w = new World();
            w.reset(1000 + r, r == 0);
            final int[] stumbles = {0};
            w.listener = new World.Listener() {
                public void onEvent(int ev, int arg) { if (ev == World.EV_STUMBLE) stumbles[0]++; }
            };
            w.pregen();
            w.started = true;
            botTick = 0;
            float t = 0;
            java.util.Random pr = new java.util.Random(r);
            while (w.alive && t < seconds) {
                if (smart) botSmart(w); else botStep(w);
                if (pr.nextFloat() < 0.0015f) w.activate(pr.nextInt(4));
                w.update(1f / 60f);
                t += 1f / 60f;
            }
            totalDist += w.dist;
            if (w.alive) survived++;
            else if (w.cause == World.CAUSE_CRASH) crash++;
            else caught++;
            if (!w.alive) {
                System.out.printf("run %2d morreu (%s) em %.0fs, dist %.0f m, lane %d, px %.2f py %.2f, tropecos %d, speed %.1f%n",
                        r, w.cause == World.CAUSE_CRASH ? "batida" : "pato", t, w.dist, w.lane, w.px, w.py, stumbles[0], w.speed);
                for (World.Obs o : w.obs) {
                    if (!o.alive) continue;
                    float d0 = o.g0 - w.P, d1 = o.g1 - w.P;
                    if (d1 < -3 || d0 > 25) continue;
                    System.out.printf("     obs tipo=%d lane=%d g=[%.1f, %.1f] mov=%b%n", o.type, o.lane, d0, d1, o.moving);
                }
            } else {
                System.out.printf("run %2d sobreviveu %.0fs, dist %.0f m, tropecos %d%n", r, t, w.dist, stumbles[0]);
            }
        }
        System.out.printf("RESUMO: %d corridas, %d sobreviveram, %d batidas, %d pegos pelo pato, distancia media %.0f m%n",
                runs, survived, crash, caught, totalDist / runs);
    }

    // ------------------------------------------------------------------ audio
    static void audio(String out) throws Exception {
        long t0 = System.nanoTime();
        short[][] fx = Synth.effects();
        long t1 = System.nanoTime();
        short[] mus = Synth.music();
        long t2 = System.nanoTime();
        System.out.printf("efeitos: %.0f ms, musica: %.0f ms (%.1f s)%n", (t1 - t0) / 1e6, (t2 - t1) / 1e6, mus.length / (float) Synth.RATE);
        String[] names = {"coin", "jump", "slide", "lane", "land", "stumble", "crash", "power", "quack", "milestone", "click", "gameover", "buy", "superjump"};
        for (int i = 0; i < fx.length; i++) wav(out + "/" + names[i] + ".wav", fx[i]);
        wav(out + "/music.wav", mus);
    }

    static void wav(String path, short[] s) throws Exception {
        java.io.DataOutputStream d = new java.io.DataOutputStream(new java.io.FileOutputStream(path));
        int bytes = s.length * 2;
        d.writeBytes("RIFF"); d.writeInt(Integer.reverseBytes(36 + bytes)); d.writeBytes("WAVEfmt ");
        d.writeInt(Integer.reverseBytes(16)); d.writeShort(Short.reverseBytes((short) 1)); d.writeShort(Short.reverseBytes((short) 1));
        d.writeInt(Integer.reverseBytes(Synth.RATE)); d.writeInt(Integer.reverseBytes(Synth.RATE * 2));
        d.writeShort(Short.reverseBytes((short) 2)); d.writeShort(Short.reverseBytes((short) 16));
        d.writeBytes("data"); d.writeInt(Integer.reverseBytes(bytes));
        int peak = 0;
        for (short v : s) { d.writeShort(Short.reverseBytes(v)); peak = Math.max(peak, Math.abs(v)); }
        d.close();
        System.out.printf("  %s  %.2fs pico %d%n", path, s.length / (float) Synth.RATE, peak);
    }
}
