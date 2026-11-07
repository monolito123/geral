package com.monolito.correlula;

import java.util.Random;

/**
 * Simulacao do corredor infinito (sem dependencias de Android).
 * Eixo x = lateral, y = altura, distancia "g" cresce para frente. O jogador fica em g = P.
 */
final class World {
    interface Listener {
        void onEvent(int ev, int arg);
    }

    static final int EV_COIN = 1, EV_JUMP = 2, EV_SLIDE = 3, EV_LANE = 4, EV_LAND = 5, EV_STUMBLE = 6,
            EV_CRASH = 7, EV_CAUGHT = 8, EV_POWER = 9, EV_POWER_END = 10, EV_MILESTONE = 11, EV_BUMP = 12,
            EV_SUPERJUMP = 13, EV_ONBUS = 14, EV_COINS100 = 15;

    static final int SW_LEFT = 0, SW_RIGHT = 1, SW_UP = 2, SW_DOWN = 3;

    static final float LANE_W = 2.5f;
    static final float BUS_HW = 1.05f, BAR_HW = 1.1f, BUS_H = 2.6f;
    static final float CAR_LEN = 10.5f, RAMP_LEN = 8f;
    static final float STEP = 0.5f;
    static final float PHW = 0.3f, PHD = 0.35f;
    static final float STAND_H = 1.65f, SLIDE_H = 0.75f;
    static final float LOW_TOP = 0.95f, HIGH_BOT = 1.2f, HIGH_TOP = 3.0f;
    static final float ACT = 110f;
    static final float JET_Y = 7f;

    static final int O_LOW = 0, O_HIGH = 1, O_BUS = 2, O_RAMP = 3;
    static final int PW_MAGNET = 0, PW_JET = 1, PW_SHOES = 2, PW_MULT = 3;
    static final int CAUSE_CRASH = 0, CAUSE_CAUGHT = 1;

    static final class Obs {
        int type, lane, variant, cars;
        float x, g0, g1, ms;
        boolean moving, act, alive;
    }

    static final class Coin {
        float x, y, g;
        boolean alive, mag;
    }

    static final class Pow {
        int kind;
        float x, y, g;
        boolean alive;
    }

    final Obs[] obs = new Obs[96];
    final Coin[] coins = new Coin[480];
    final Pow[] pows = new Pow[8];
    {
        for (int i = 0; i < obs.length; i++) obs[i] = new Obs();
        for (int i = 0; i < coins.length; i++) coins[i] = new Coin();
        for (int i = 0; i < pows.length; i++) pows[i] = new Pow();
    }

    Listener listener;
    Random rng = new Random();
    final float[] powDur = {10f, 8f, 10f, 12f};

    // jogador
    float P, dist, speed, score, rebase;
    int lane;
    float px, py, vy, prevX, prevY, prevP;
    boolean grounded, alive, started;
    float slideT, coyote, jumpBuf, stumbleT, invulnT, deathT, runT;
    boolean slideQueued;
    int cause, coinsRun, revives, nextMilestone, coinMark;
    final float[] powT = new float[4];
    float duckD = 2.6f, duckX;
    float runPhase, lean;
    float surf; // altura da superficie sob o jogador

    // gerador
    float frontier;
    final float[] busy = new float[3];
    int lastBlock = -1;
    boolean tutorial;

    void reset(long seed, boolean tutorial) {
        rng = new Random(seed);
        this.tutorial = tutorial;
        for (Obs o : obs) o.alive = false;
        for (Coin c : coins) c.alive = false;
        for (Pow p : pows) p.alive = false;
        P = 0; dist = 0; score = 0; rebase = 0;
        lane = 0; px = 0; py = 0; vy = 0; prevX = 0; prevY = 0; prevP = 0;
        grounded = true; alive = true; started = false;
        slideT = 0; coyote = 0; jumpBuf = 0; stumbleT = 0; invulnT = 0; deathT = 0; runT = 0;
        slideQueued = false;
        cause = 0; coinsRun = 0; revives = 0; nextMilestone = 500; coinMark = 100;
        for (int i = 0; i < 4; i++) powT[i] = 0;
        duckD = 2.6f; duckX = 0;
        speed = baseSpeed(0);
        frontier = 45f;
        busy[0] = busy[1] = busy[2] = 0;
        lastBlock = -1;
        // linha de picanhas inicial
        coinLine(0, 14f, 10, 2.4f, 0.9f);
    }

    static float baseSpeed(float d) {
        return 13f + 17f * (1f - (float) Math.exp(-d / 3000f));
    }

    float gravity() {
        return 44f + (speed - 13f) * 1.3f;
    }

    float slideDur() {
        return 0.62f * (float) Math.sqrt(13f / speed);
    }

    boolean sliding() {
        return slideT > 0;
    }

    boolean jet() {
        return powT[PW_JET] > 0;
    }

    float mult() {
        return powT[PW_MULT] > 0 ? 2f : 1f;
    }

    private void emit(int ev, int arg) {
        if (listener != null) listener.onEvent(ev, arg);
    }

    // ------------------------------------------------------------------ entrada
    void swipe(int dir) {
        if (!alive) return;
        started = true;
        switch (dir) {
            case SW_LEFT:
                if (lane > -1) { lane--; emit(EV_LANE, -1); } else emit(EV_BUMP, 0);
                break;
            case SW_RIGHT:
                if (lane < 1) { lane++; emit(EV_LANE, 1); } else emit(EV_BUMP, 0);
                break;
            case SW_UP:
                jump();
                break;
            case SW_DOWN:
                slide();
                break;
            default:
                break;
        }
    }

    private void jump() {
        if (jet()) return;
        if (grounded || coyote > 0) {
            boolean sup = powT[PW_SHOES] > 0;
            float h = sup ? 4.1f : 1.95f;
            vy = (float) Math.sqrt(2f * gravity() * h);
            grounded = false;
            coyote = 0;
            slideT = 0;
            slideQueued = false;
            emit(sup ? EV_SUPERJUMP : EV_JUMP, 0);
        } else {
            jumpBuf = 0.16f;
        }
    }

    private void slide() {
        if (jet()) return;
        if (grounded) {
            slideT = slideDur();
            emit(EV_SLIDE, 0);
        } else {
            vy = Math.min(vy, -30f);
            slideQueued = true;
            jumpBuf = 0;
        }
    }

    // ------------------------------------------------------------------ passo
    void update(float dt) {
        if (dt > 0.1f) dt = 0.1f;
        int n = (int) Math.ceil(dt / 0.0085f);
        if (n < 1) n = 1;
        float h = dt / n;
        for (int i = 0; i < n; i++) step(h);
    }

    private void step(float h) {
        runT += h;
        // pato
        float dt;
        if (!alive) dt = cause == CAUSE_CAUGHT ? 0.9f : 1.7f;
        else if (runT < 2.0f || stumbleT > 0) dt = 2.2f;
        else dt = 32f;
        float k = dt > duckD ? 0.7f : 6f;
        duckD += (dt - duckD) * Math.min(1f, k * h);
        duckX += (px - duckX) * Math.min(1f, 6f * h);

        if (!alive) {
            deathT += h;
            return;
        }

        speed = baseSpeed(dist);
        prevP = P;
        prevX = px;
        prevY = py;
        float mv = speed * h;
        P += mv;
        dist += mv;
        score += mv * mult();
        runPhase += mv * 0.62f;

        if (dist >= nextMilestone) {
            emit(EV_MILESTONE, nextMilestone);
            nextMilestone += 500;
        }

        // movimento lateral
        float tx = lane * LANE_W;
        float ls = (15f + speed * 0.3f) * h;
        float dx = tx - px;
        if (Math.abs(dx) <= ls) px = tx; else px += Math.signum(dx) * ls;
        lean += ((dx > 0.05f ? -1f : (dx < -0.05f ? 1f : 0f)) * 0.3f - lean) * Math.min(1f, 12f * h);

        // obstaculos moveis
        for (Obs o : obs) {
            if (!o.alive || !o.moving) continue;
            if (!o.act && o.g0 - P < ACT) o.act = true;
            if (o.act) { o.g0 -= o.ms * h; o.g1 -= o.ms * h; }
        }

        // vertical
        surf = surfaceAt(px, P);
        if (jet()) {
            py += (JET_Y - py) * Math.min(1f, 3f * h);
            vy = 0;
            grounded = false;
            slideT = 0;
        } else {
            vy -= gravity() * h;
            py += vy * h;
            if (py <= surf) {
                if (surf - py <= STEP || prevY >= surf - STEP) {
                    if (!grounded) {
                        if (vy < -6f) emit(EV_LAND, surf > 1f ? 1 : 0);
                        if (surf > 1f) emit(EV_ONBUS, 0);
                        if (slideQueued) { slideT = slideDur(); slideQueued = false; emit(EV_SLIDE, 0); }
                    }
                    py = surf;
                    vy = 0;
                    grounded = true;
                    coyote = 0.09f;
                    if (jumpBuf > 0) { jumpBuf = 0; jump(); }
                } else if (py <= 0f) {
                    // dentro de um obstaculo (ex.: invencivel): fica no asfalto
                    py = 0f;
                    vy = 0;
                    grounded = true;
                }
            } else if (py > surf + 0.02f) {
                if (grounded) grounded = false;
            }
            if (!grounded) coyote -= h;
        }
        if (jumpBuf > 0) jumpBuf -= h;
        if (slideT > 0) slideT -= h;

        // colisoes
        if (invulnT > 0) invulnT -= h;
        else if (!jet()) collide(h);

        // power-ups
        for (int i = 0; i < 4; i++) {
            if (powT[i] > 0) {
                powT[i] -= h;
                if (powT[i] <= 0) {
                    powT[i] = 0;
                    if (i == PW_JET) invulnT = 1.3f;
                    emit(EV_POWER_END, i);
                }
            }
        }
        if (stumbleT > 0) stumbleT -= h;

        if (sim) return;
        coinsAndPows(h);
        cleanupAndGenerate();
    }

    /** Modo simulacao (testes): ignora coletaveis e geracao. */
    boolean sim;
    Obs lastHit;

    /** Copia o estado de outro mundo (usado pelo bot de testes). */
    void copyFrom(World s) {
        P = s.P; dist = s.dist; speed = s.speed; score = s.score; rebase = s.rebase;
        lane = s.lane; px = s.px; py = s.py; vy = s.vy; prevX = s.prevX; prevY = s.prevY; prevP = s.prevP;
        grounded = s.grounded; alive = s.alive; started = s.started;
        slideT = s.slideT; coyote = s.coyote; jumpBuf = s.jumpBuf; stumbleT = s.stumbleT; invulnT = s.invulnT;
        deathT = s.deathT; runT = s.runT; slideQueued = s.slideQueued; cause = s.cause;
        System.arraycopy(s.powT, 0, powT, 0, 4);
        System.arraycopy(s.powDur, 0, powDur, 0, 4);
        duckD = s.duckD; duckX = s.duckX; surf = s.surf; lean = s.lean;
        for (int i = 0; i < obs.length; i++) {
            Obs a = obs[i], b = s.obs[i];
            a.type = b.type; a.lane = b.lane; a.variant = b.variant; a.cars = b.cars;
            a.x = b.x; a.g0 = b.g0; a.g1 = b.g1; a.ms = b.ms; a.moving = b.moving; a.act = b.act; a.alive = b.alive;
        }
    }

    float surfaceAt(float x, float g) {
        float s = 0f;
        for (Obs o : obs) {
            if (!o.alive || (o.type != O_BUS && o.type != O_RAMP)) continue;
            if (g < o.g0 - 0.05f || g > o.g1 + 0.05f) continue;
            if (Math.abs(x - o.x) >= BUS_HW) continue;
            float hh = o.type == O_BUS ? BUS_H : BUS_H * clamp((g - o.g0) / (o.g1 - o.g0), 0f, 1f);
            if (hh > s) s = hh;
        }
        return s;
    }

    static float clamp(float v, float a, float b) {
        return v < a ? a : (v > b ? b : v);
    }

    private void collide(float h) {
        float pz0 = P - PHD, pz1 = P + PHD;
        float qz0 = prevP - PHD, qz1 = prevP + PHD;
        float hgt = sliding() ? SLIDE_H : STAND_H;
        float pBot = py, pTop = py + hgt;
        float qBot = prevY, qTop = prevY + hgt;
        for (Obs o : obs) {
            if (!o.alive) continue;
            if (o.g1 < pz0 - 1f || o.g0 > pz1 + 1f) continue;
            float hw = (o.type == O_BUS || o.type == O_RAMP) ? BUS_HW : BAR_HW;
            float lim = hw + PHW;
            if (Math.abs(px - o.x) >= lim) continue;
            if (o.g1 < pz0 || o.g0 > pz1) continue;
            float oBot, oTop;
            switch (o.type) {
                case O_LOW: oBot = 0; oTop = LOW_TOP; break;
                case O_HIGH: oBot = HIGH_BOT; oTop = HIGH_TOP; break;
                case O_BUS: oBot = 0; oTop = BUS_H - STEP; break;
                default: oBot = 0; oTop = BUS_H * clamp((P - o.g0) / (o.g1 - o.g0), 0f, 1f) - STEP; break;
            }
            if (oTop <= oBot) continue;
            if (pTop <= oBot || pBot >= oTop) continue;
            // houve contato: descobre de onde veio
            float mvG = o.moving && o.act ? o.ms * h : 0f;
            boolean prevZ = !(o.g1 + mvG < qz0 || o.g0 + mvG > qz1);
            boolean prevXo = Math.abs(prevX - o.x) < lim;
            boolean prevYo = !(qTop <= oBot || qBot >= oTop);
            lastHit = o;
            if (!prevZ) {
                if (prevXo) { crash(); return; }
                sideHit(); return;
            }
            if (!prevXo) { sideHit(); return; }
            if (!prevYo) {
                if (o.type == O_HIGH && qTop <= oBot + 0.01f) {
                    // levantou embaixo da faixa: continua abaixado
                    slideT = Math.max(slideT, 0.08f);
                }
                continue;
            }
        }
    }

    private void sideHit() {
        px = prevX;
        int from = Math.round(prevX / LANE_W);
        if (from < -1) from = -1;
        if (from > 1) from = 1;
        lane = from;
        if (stumbleT > 0) {
            caught();
        } else {
            stumbleT = 3.2f;
            emit(EV_STUMBLE, 0);
        }
    }

    private void crash() {
        alive = false;
        cause = CAUSE_CRASH;
        deathT = 0;
        slideT = 0;
        emit(EV_CRASH, 0);
    }

    private void caught() {
        alive = false;
        cause = CAUSE_CAUGHT;
        deathT = 0;
        emit(EV_CAUGHT, 0);
    }

    void revive() {
        alive = true;
        revives++;
        invulnT = 2.5f;
        stumbleT = 0;
        deathT = 0;
        slideT = 0;
        vy = 0;
        for (Obs o : obs) {
            if (o.alive && o.g1 > P - 3f && o.g0 < P + 45f) o.alive = false;
        }
        if (frontier < P + 45f) frontier = P + 45f;
        py = surfaceAt(px, P);
        duckD = 6f;
    }

    // ------------------------------------------------------------------ coletaveis
    private void coinsAndPows(float h) {
        boolean mag = powT[PW_MAGNET] > 0;
        float cy = py + 0.8f;
        for (Coin c : coins) {
            if (!c.alive) continue;
            float dg = c.g - P;
            if (dg > 30f) continue;
            if (mag && !c.mag && dg < 16f && dg > -1.5f) c.mag = true;
            if (c.mag) {
                float ax = px - c.x, ay = cy - c.y, ag = P - c.g;
                float len = (float) Math.sqrt(ax * ax + ay * ay + ag * ag);
                float st = (26f + speed) * h;
                if (len <= st || len < 0.5f) { c.x = px; c.y = cy; c.g = P; }
                else { float t = st / len; c.x += ax * t; c.y += ay * t; c.g += ag * t; }
            }
            if (Math.abs(c.x - px) < 0.95f && Math.abs(c.y - cy) < 1.25f && c.g > prevP - 0.8f && c.g < P + 0.8f) {
                c.alive = false;
                coinsRun++;
                emit(EV_COIN, coinsRun);
                if (coinsRun >= coinMark) { emit(EV_COINS100, coinMark); coinMark += 100; }
            }
        }
        for (Pow p : pows) {
            if (!p.alive) continue;
            if (Math.abs(p.x - px) < 1.0f && Math.abs(p.y - cy) < 1.4f && p.g > prevP - 0.9f && p.g < P + 0.9f) {
                p.alive = false;
                activate(p.kind);
            }
        }
    }

    void activate(int kind) {
        powT[kind] = powDur[kind];
        if (kind == PW_JET) {
            slideT = 0;
            // picanhas no ar
            float g = P + 14f;
            float end = P + 14f + speed * (powDur[kind] - 0.8f);
            int ln = lane;
            while (g < end) {
                int cnt = 6;
                coinLine(ln, g, cnt, 2.6f, JET_Y + 0.8f);
                g += cnt * 2.6f + 1.5f;
                int nl = ln + (rng.nextBoolean() ? 1 : -1);
                if (nl < -1) nl = 0;
                if (nl > 1) nl = 0;
                ln = nl;
            }
        }
        emit(EV_POWER, kind);
    }

    // ------------------------------------------------------------------ gerador
    private void cleanupAndGenerate() {
        for (Obs o : obs) if (o.alive && o.g1 < P - 20f) o.alive = false;
        for (Coin c : coins) if (c.alive && c.g < P - 6f) c.alive = false;
        for (Pow p : pows) if (p.alive && p.g < P - 6f) p.alive = false;
        while (frontier < P + 175f) genBlock();
        if (P > 900f) {
            float d = 900f;
            P -= d; prevP -= d; frontier -= d; rebase += d;
            for (int i = 0; i < 3; i++) busy[i] -= d;
            for (Obs o : obs) { o.g0 -= d; o.g1 -= d; }
            for (Coin c : coins) c.g -= d;
            for (Pow p : pows) p.g -= d;
        }
    }

    /** Preenche o caminho a frente (tambem usado no menu). */
    void pregen() {
        while (frontier < P + 175f) genBlock();
    }

    private int diff(float d) {
        if (d < 300) return 0;
        if (d < 1200) return 1;
        if (d < 2800) return 2;
        return 3;
    }

    private Obs addObs(int type, int lane, float g0, float g1) {
        for (Obs o : obs) {
            if (!o.alive) {
                o.alive = true;
                o.type = type;
                o.lane = lane;
                o.x = lane * LANE_W;
                o.g0 = g0;
                o.g1 = g1;
                o.moving = false;
                o.act = false;
                o.ms = 0;
                o.cars = 1;
                o.variant = rng.nextInt(1 << 16);
                return o;
            }
        }
        return null;
    }

    void coinLine(int lane, float g, int n, float sp, float y) {
        for (int i = 0; i < n; i++) addCoin(lane * LANE_W, y, g + i * sp);
    }

    /** Picanhas subindo a rampa. */
    private void coinRamp(int lane, float g0) {
        for (int i = 0; i < 4; i++) {
            float t = (i + 0.5f) / 4f;
            addCoin(lane * LANE_W, BUS_H * t + 0.9f, g0 + RAMP_LEN * t);
        }
    }

    private void addCoin(float x, float y, float g) {
        for (Coin c : coins) {
            if (!c.alive) {
                c.alive = true;
                c.mag = false;
                c.x = x; c.y = y; c.g = g;
                return;
            }
        }
    }

    private void coinArc(int lane, float gc, float v) {
        float g = gravity();
        float vy0 = (float) Math.sqrt(2f * g * 1.95f);
        float air = 2f * vy0 / g;
        float half = v * air * 0.5f;
        int n = 7;
        for (int i = 0; i < n; i++) {
            float t = -1f + 2f * i / (n - 1);
            float y = 0.9f + 1.85f * (1f - t * t);
            addCoin(lane * LANE_W, y, gc + t * half * 0.85f);
        }
    }

    private void addPow(int lane, float g) {
        int kind = -1;
        for (int tries = 0; tries < 6; tries++) {
            int k = rng.nextInt(4);
            if (powT[k] <= 0) { kind = k; break; }
        }
        if (kind < 0) return;
        for (Pow p : pows) {
            if (!p.alive) {
                p.alive = true;
                p.kind = kind;
                p.x = lane * LANE_W;
                p.y = 1.1f;
                p.g = g;
                return;
            }
        }
    }

    private boolean free(int ln, float g) {
        return busy[ln + 1] < g;
    }

    private int randFreeLane(float g) {
        int s = rng.nextInt(3);
        for (int i = 0; i < 3; i++) {
            int l = (s + i) % 3 - 1;
            if (free(l, g)) return l;
        }
        return 0;
    }

    private static final int B_COINS = 0, B_BARRIERS = 1, B_BUSES = 2, B_MOVING = 3;

    private void genBlock() {
        float ahead = dist + (frontier - P);
        float v = baseSpeed(ahead);
        float win = Math.max(11f, v * 0.85f);
        float z = frontier + win;
        int d = tutorial && ahead < 260 ? -1 : diff(ahead);
        boolean anyBusy = !free(-1, z) || !free(0, z) || !free(1, z);

        if (d < 0) { tutorialBlock(z); return; }

        int t;
        int[] w;
        switch (d) {
            case 0: w = new int[]{30, 45, 25, 0}; break;
            case 1: w = new int[]{20, 40, 28, 12}; break;
            case 2: w = new int[]{14, 37, 31, 18}; break;
            default: w = new int[]{10, 36, 34, 20}; break;
        }
        if (anyBusy) { w[B_BUSES] = 0; w[B_MOVING] = 0; }
        if (lastBlock == B_COINS) w[B_COINS] = 0;
        int sum = w[0] + w[1] + w[2] + w[3];
        int r = rng.nextInt(sum);
        t = 0;
        while (r >= w[t]) { r -= w[t]; t++; }
        lastBlock = t;

        switch (t) {
            case B_COINS: blockCoins(z, d); break;
            case B_BARRIERS: blockBarriers(z, d, v); break;
            case B_BUSES: blockBuses(z, d, v); break;
            default: blockMoving(z, d, v); break;
        }
        // power-up ocasional
        if (dist > 120f && rng.nextFloat() < 0.075f) {
            int ln = randFreeLane(frontier + 3f);
            if (free(ln, frontier + 3f)) addPow(ln, frontier + 3f);
            frontier += 6f;
        }
    }

    private int tutStage;

    private void tutorialBlock(float z) {
        // sequencia ensinando: desviar, pular, abaixar
        switch (tutStage % 4) {
            case 0: { // onibus no meio: desviar
                Obs o = addObs(O_BUS, 0, z, z + CAR_LEN * 2);
                if (o != null) o.cars = 2;
                busy[1] = z + CAR_LEN * 2;
                coinLine(-1, z, 8, 2.6f, 0.9f);
                frontier = z + CAR_LEN * 2;
                break;
            }
            case 1: // barreiras baixas: pular
                for (int l = -1; l <= 1; l++) addObs(O_LOW, l, z, z + 0.35f);
                coinArc(0, z, baseSpeed(dist));
                frontier = z + 4f;
                break;
            case 2: // faixas altas: abaixar
                for (int l = -1; l <= 1; l++) addObs(O_HIGH, l, z, z + 0.3f);
                coinLine(0, z - 4f, 5, 2f, 0.6f);
                frontier = z + 4f;
                break;
            default: { // rampa + onibus
                Obs ra = addObs(O_RAMP, 1, z, z + RAMP_LEN);
                Obs o = addObs(O_BUS, 1, z + RAMP_LEN, z + RAMP_LEN + CAR_LEN * 2);
                if (o != null) o.cars = 2;
                busy[2] = z + RAMP_LEN + CAR_LEN * 2;
                coinRamp(1, z);
                coinLine(1, z + RAMP_LEN + 1f, 8, 2.5f, BUS_H + 0.9f);
                frontier = z + RAMP_LEN + CAR_LEN * 2;
                if (ra == null) frontier = z;
                break;
            }
        }
        tutStage++;
    }

    private void blockCoins(float z, int d) {
        int ln = randFreeLane(z);
        int n = 8 + rng.nextInt(7);
        coinLine(ln, z, n, 2.3f, 0.9f);
        frontier = z + n * 2.3f;
    }

    private void blockBarriers(float z, int d, float v) {
        int rows = d == 0 ? 1 : 1 + rng.nextInt(d >= 2 ? 3 : 2);
        float sp = Math.max(9.5f, v * 0.8f);
        for (int r = 0; r < rows; r++) {
            float zr = z + r * sp;
            int count = 0;
            int[] kind = new int[3];
            for (int i = 0; i < 3; i++) {
                int l = i - 1;
                if (!free(l, zr + 1f)) { kind[i] = -2; continue; }
                float x = rng.nextFloat();
                kind[i] = x < 0.4f ? O_LOW : (x < 0.75f ? O_HIGH : -1);
                if (kind[i] >= 0) count++;
            }
            if (count == 0) {
                for (int i = 0; i < 3; i++) if (kind[i] == -1) { kind[i] = O_LOW; break; }
            }
            if (d == 0 && count == 3) kind[rng.nextInt(3)] = -1;
            boolean coinsDone = false;
            for (int i = 0; i < 3; i++) {
                int l = i - 1;
                if (kind[i] == O_LOW) {
                    addObs(O_LOW, l, zr, zr + 0.35f);
                    if (!coinsDone && rng.nextFloat() < 0.6f) { coinArc(l, zr + 0.2f, v); coinsDone = true; }
                } else if (kind[i] == O_HIGH) {
                    addObs(O_HIGH, l, zr, zr + 0.3f);
                    if (!coinsDone && rng.nextFloat() < 0.35f) { coinLine(l, zr - 3f, 4, 2f, 0.6f); coinsDone = true; }
                } else if (kind[i] == -1 && !coinsDone && rng.nextFloat() < 0.5f) {
                    coinLine(l, zr - 4f, 5, 2f, 0.9f);
                    coinsDone = true;
                }
            }
        }
        frontier = z + (rows - 1) * sp + 2f;
    }

    private void blockBuses(float z, int d, float v) {
        int n;
        if (d == 0) n = 1;
        else if (d == 1) n = rng.nextFloat() < 0.6f ? 1 : 2;
        else { float x = rng.nextFloat(); n = x < 0.3f ? 1 : (x < 0.75f ? 2 : 3); }
        boolean[] use = new boolean[3];
        int placed = 0;
        while (placed < n) {
            int i = rng.nextInt(3);
            if (!use[i]) { use[i] = true; placed++; }
        }
        int rampLane = -9;
        if (n == 3 || rng.nextFloat() < 0.45f) {
            int s = rng.nextInt(3);
            for (int i = 0; i < 3; i++) {
                int c = (s + i) % 3;
                if (use[c]) { rampLane = c - 1; break; }
            }
        }
        float zs = rampLane != -9 ? z + RAMP_LEN : z;
        float maxEnd = zs;
        for (int i = 0; i < 3; i++) {
            int l = i - 1;
            if (!use[i]) continue;
            int cars = 1 + rng.nextInt(d >= 1 ? 3 : 2);
            if (l == rampLane && cars < 2) cars = 2;
            float g0 = zs, g1 = zs + cars * CAR_LEN;
            Obs o = addObs(O_BUS, l, g0, g1);
            if (o != null) o.cars = cars;
            busy[i] = g1 + 1f;
            if (g1 > maxEnd) maxEnd = g1;
            if (l == rampLane) {
                addObs(O_RAMP, l, z, zs);
                coinRamp(l, z);
                coinLine(l, zs + 1.5f, (int) ((g1 - zs - 2f) / 2.5f), 2.5f, BUS_H + 0.9f);
            }
        }
        // faixas livres: picanhas e talvez barreiras
        for (int i = 0; i < 3; i++) {
            if (use[i]) continue;
            int l = i - 1;
            float len = maxEnd - z;
            if (d >= 1 && len > 18f && rng.nextFloat() < 0.6f) {
                float zb = z + len * 0.5f;
                int k = rng.nextBoolean() ? O_LOW : O_HIGH;
                addObs(k, l, zb, zb + (k == O_LOW ? 0.35f : 0.3f));
                if (k == O_LOW) coinArc(l, zb + 0.2f, v);
            } else if (rng.nextFloat() < 0.7f) {
                coinLine(l, z + 2f, Math.max(3, (int) (len / 2.6f) - 1), 2.5f, 0.9f);
            }
        }
        frontier = maxEnd + 1f;
    }

    private void blockMoving(float z, int d, float v) {
        int L = rng.nextInt(3) - 1;
        float ms = 7f + d * 1.5f;
        int cars = rng.nextFloat() < 0.6f ? 1 : 2;
        float len = cars * CAR_LEN;
        float sweep = ACT * ms / (v + ms) * 1.12f + 3f;
        float g0 = z + sweep;
        Obs o = addObs(O_BUS, L, g0, g0 + len);
        if (o != null) {
            o.moving = true;
            o.ms = ms;
            o.cars = cars;
        }
        busy[L + 1] = g0 + len + 2f;
        // outras faixas
        int o1 = L == -1 ? 0 : -1, o2 = L == 1 ? 0 : 1;
        if (L == 0) { o1 = -1; o2 = 1; }
        float x = rng.nextFloat();
        if (d >= 2 && x < 0.35f) {
            // onibus parado numa lateral, a outra livre
            int sl = rng.nextBoolean() ? o1 : o2;
            int fl = sl == o1 ? o2 : o1;
            float sg0 = z + 2f;
            boolean ramp = rng.nextFloat() < 0.5f;
            float bs = ramp ? sg0 + RAMP_LEN : sg0;
            Obs b = addObs(O_BUS, sl, bs, bs + CAR_LEN * 2);
            if (b != null) b.cars = 2;
            if (ramp) {
                addObs(O_RAMP, sl, sg0, bs);
                coinLine(sl, bs + 1.5f, 7, 2.5f, BUS_H + 0.9f);
            }
            busy[sl + 1] = bs + CAR_LEN * 2 + 1f;
            coinLine(fl, z + 3f, 8, 2.4f, 0.9f);
        } else {
            float zb = z + sweep * 0.45f;
            int k = rng.nextBoolean() ? O_LOW : O_HIGH;
            int bl = rng.nextBoolean() ? o1 : o2;
            addObs(k, bl, zb, zb + 0.35f);
            if (k == O_LOW) coinArc(bl, zb + 0.2f, v);
            int cl = bl == o1 ? o2 : o1;
            coinLine(cl, z + 2f, 7, 2.4f, 0.9f);
        }
        frontier = z + Math.max(sweep * 0.6f, 14f);
    }
}
