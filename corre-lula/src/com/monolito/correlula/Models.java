package com.monolito.correlula;

/** Modelos 3D low-poly gerados por codigo (nenhum arquivo de modelo no APK). */
final class Models {
    Gfx.Mesh cube, coin, low, high, ramp, plane, duckBody, duckWing, duckLeg, backdrop, sash, train, trainCar;
    final Gfx.Mesh[] bus = new Gfx.Mesh[4];
    final Gfx.Mesh[] leg = new Gfx.Mesh[5], arm = new Gfx.Mesh[5], torso = new Gfx.Mesh[5], head = new Gfx.Mesh[5];

    static final int[] BUS_COLORS = {0x1D5BD8, 0x168A3E, 0xD62828, 0x7C3AED};

    // paletas das roupas: jaqueta, calca, camisa, gravata, sapato
    static final int[][] SKINS = {
            {0x1E2A44, 0x1B2438, 0xF4F4F4, 0xD01C1F, 0x151515},
            {0x2F5FA8, 0x2F5FA8, 0x9AA3AD, 0x2F5FA8, 0x6B3F1D},
            {0xC8102E, 0x3B5B8A, 0xC8102E, 0xFFFFFF, 0xEDEDED},
            {0xF2F2F2, 0x18181B, 0xF2F2F2, 0x18181B, 0x18181B},
            {0x111A2A, 0x111A2A, 0xF4F4F4, 0x111A2A, 0x101010},
    };
    static final int SKIN = 0xD9A066, HAIR = 0xCFCFCF, BEARD = 0xE6E6E6;

    void build(Gfx g, MeshBuilder mb) {
        int A = MeshBuilder.ALL;

        mb.reset();
        mb.box(-0.5f, -0.5f, -0.5f, 0.5f, 0.5f, 0.5f, 0xFFFFFF, A);
        cube = mb.build(g);

        // picanha no espeto
        mb.reset();
        mb.box(-0.025f, -0.45f, -0.025f, 0.025f, 0.48f, 0.025f, 0xD1D5DB, A);
        mb.box(-0.32f, -0.22f, -0.1f, 0.32f, 0.12f, 0.1f, 0xC53A2E, A);
        mb.box(-0.34f, 0.12f, -0.115f, 0.34f, 0.27f, 0.115f, 0xF6E4C2, A);
        for (int s = -1; s <= 1; s += 2) {
            float z0 = s > 0 ? 0.1f : -0.112f, z1 = s > 0 ? 0.112f : -0.1f;
            mb.box(-0.22f, -0.16f, z0, -0.15f, 0.06f, z1, 0x7A1D17, A);
            mb.box(0.02f, -0.16f, z0, 0.09f, 0.06f, z1, 0x7A1D17, A);
        }
        coin = mb.build(g);

        buildBarriers(g, mb);
        for (int i = 0; i < 4; i++) bus[i] = buildBus(g, mb, BUS_COLORS[i]);
        buildTrain(g, mb);
        buildRamp(g, mb);
        buildPlane(g, mb);
        buildDuck(g, mb);
        for (int i = 0; i < SKINS.length; i++) buildLula(g, mb, i);
        buildSash(g, mb);
        buildBackdrop(g, mb);
    }

    private void buildBarriers(Gfx g, MeshBuilder mb) {
        int A = MeshBuilder.ALL;
        mb.reset();
        // cavalete de obra (pular)
        mb.box(-1.06f, 0f, -0.33f, -0.94f, 0.95f, -0.02f, 0xE5E7EB, A);
        mb.box(0.94f, 0f, -0.33f, 1.06f, 0.95f, -0.02f, 0xE5E7EB, A);
        for (int i = 0; i < 6; i++) {
            float x0 = -1.1f + i * (2.2f / 6f), x1 = x0 + 2.2f / 6f;
            mb.box(x0, 0.5f, -0.14f, x1, 0.95f, 0f, (i % 2 == 0) ? 0xF97316 : 0xF8FAFC, A);
            mb.box(x0, 0.12f, -0.14f, x1, 0.3f, 0f, (i % 2 == 0) ? 0xF8FAFC : 0xF97316, A);
        }
        mb.box(-0.56f, 0.56f, 0f, 0.56f, 0.89f, 0.02f, 0xFFFFFF, A);
        low = mb.build(g);

        // faixa alta (abaixar): postes + moldura; o painel colorido e o texto vem do batch
        mb.reset();
        mb.box(-1.22f, 0f, -0.2f, -1.1f, 3.05f, -0.08f, 0x6B7280, A);
        mb.box(1.1f, 0f, -0.2f, 1.22f, 3.05f, -0.08f, 0x6B7280, A);
        mb.box(-1.22f, 2.95f, -0.2f, 1.22f, 3.07f, -0.08f, 0x6B7280, A);
        mb.box(-1.1f, World.HIGH_BOT + 0.03f, -0.12f, 1.1f, 2.93f, -0.01f, 0xF8FAFC, A);
        high = mb.build(g);
    }

    private Gfx.Mesh buildBus(Gfx g, MeshBuilder mb, int col) {
        int A = MeshBuilder.ALL, V = MeshBuilder.VIS;
        float L = World.CAR_LEN - 0.4f;
        mb.reset();
        mb.box(-1.05f, 0.35f, -L, 1.05f, 2.5f, 0f, col, A);
        mb.box(-1.0f, 2.5f, -L + 0.05f, 1.0f, 2.6f, -0.05f, 0xE8EAEE, A);
        mb.box(-1.0f, 0.15f, -L + 0.1f, 1.0f, 0.35f, -0.1f, 0x2A2D33, V);
        for (int s = -1; s <= 1; s += 2) {
            float xa = s * 1.0f, xb = s * 1.09f;
            float x0 = Math.min(xa, xb), x1 = Math.max(xa, xb);
            mb.box(x0, 0f, -2.3f, x1, 0.62f, -1.3f, 0x111111, V);
            mb.box(x0, 0f, -L + 1.3f, x1, 0.62f, -L + 2.3f, 0x111111, V);
            float wa = s * 1.05f, wb = s * 1.07f;
            float w0 = Math.min(wa, wb), w1 = Math.max(wa, wb);
            for (int k = 0; k < 4; k++) {
                float z1 = -0.7f - k * 2.35f, z0 = z1 - 2.05f;
                mb.box(w0, 1.35f, z0, w1, 2.22f, z1, 0x24324A, V);
            }
            mb.box(w0, 0.85f, -L + 0.1f, w1, 1.05f, -0.1f, 0xF8FAFC, V);
        }
        mb.box(-0.92f, 1.25f, 0f, 0.92f, 2.14f, 0.02f, 0x22303F, A);
        mb.box(-0.82f, 2.19f, 0f, 0.82f, 2.46f, 0.03f, 0x0E0E10, A);
        mb.box(-0.92f, 0.5f, 0f, -0.6f, 0.72f, 0.04f, 0xF1F1D0, A);
        mb.box(0.6f, 0.5f, 0f, 0.92f, 0.72f, 0.04f, 0xF1F1D0, A);
        mb.box(-1.06f, 0.12f, 0f, 1.06f, 0.42f, 0.08f, 0x3A3D44, A);
        return mb.build(g);
    }

    /** Trem-bala (obstaculo que vem na sua direcao). */
    private void buildTrain(Gfx g, MeshBuilder mb) {
        int A = MeshBuilder.ALL, V = MeshBuilder.VIS;
        float L = World.CAR_LEN - 0.4f;
        for (int pass = 0; pass < 2; pass++) {
            mb.reset();
            boolean front = pass == 0;
            mb.box(-1.05f, 0.3f, -L, 1.05f, front ? 2.0f : 2.6f, front ? -1.4f : 0f, 0xF1F5F9, A);
            if (front) {
                // nariz chanfrado
                mb.box(-1.05f, 0.3f, -1.4f, 1.05f, 2.0f, 0f, 0xF1F5F9, A);
                mb.box(-1.05f, 2.0f, -L, 1.05f, 2.6f, -1.4f, 0xF1F5F9, A);
                mb.quad(-1.05f, 2.0f, 0f, 1.05f, 2.0f, 0f, 1.05f, 2.6f, -1.4f, -1.05f, 2.6f, -1.4f, 0xDDE3EA);
                mb.tri(1.05f, 2.0f, 0f, 1.05f, 2.0f, -1.4f, 1.05f, 2.6f, -1.4f, 0xDDE3EA);
                mb.tri(-1.05f, 2.0f, 0f, -1.05f, 2.6f, -1.4f, -1.05f, 2.0f, -1.4f, 0xDDE3EA);
                mb.box(-0.9f, 1.25f, 0f, 0.9f, 1.95f, 0.02f, 0x1F2A3A, A);
                mb.box(-0.95f, 0.45f, 0f, -0.55f, 0.75f, 0.05f, 0xFFF7AE, A);
                mb.box(0.55f, 0.45f, 0f, 0.95f, 0.75f, 0.05f, 0xFFF7AE, A);
                mb.box(-0.6f, 0.82f, 0f, 0.6f, 1.12f, 0.03f, 0x0E0E10, A);
                mb.box(-1.06f, 0.1f, 0f, 1.06f, 0.4f, 0.09f, 0xC1121F, A);
            }
            mb.box(-1.0f, 0.1f, -L + 0.1f, 1.0f, 0.3f, -0.1f, 0x2A2D33, V);
            for (int s = -1; s <= 1; s += 2) {
                float wa = s * 1.05f, wb = s * 1.07f;
                float w0 = Math.min(wa, wb), w1 = Math.max(wa, wb);
                mb.box(w0, 0.62f, -L + 0.1f, w1, 0.9f, front ? -0.3f : -0.1f, 0xC1121F, V);
                mb.box(w0, 0.9f, -L + 0.1f, w1, 1.0f, front ? -0.3f : -0.1f, 0x16A34A, V);
                mb.box(w0, 1.3f, -L + 0.5f, w1, 2.0f, front ? -1.6f : -0.5f, 0x1F2A3A, V);
            }
            if (front) train = mb.build(g); else trainCar = mb.build(g);
        }
    }

    private void buildRamp(Gfx g, MeshBuilder mb) {
        float x0 = -1.05f, x1 = 1.05f, L = World.RAMP_LEN, H = World.BUS_H;
        mb.reset();
        mb.quad(x0, 0.02f, 0f, x1, 0.02f, 0f, x1, H, -L, x0, H, -L, 0x9AA1AB);
        mb.tri(x1, 0f, 0f, x1, 0f, -L, x1, H, -L, 0x6B7280);
        mb.tri(x0, 0f, 0f, x0, H, -L, x0, 0f, -L, 0x6B7280);
        float e = 0.04f;
        for (int k = 0; k < 4; k++) {
            float t0 = 0.1f + k * 0.22f, t1 = t0 + 0.1f;
            float ya = 0.02f + H * t0 + e, yb = 0.02f + H * t1 + e;
            mb.quad(x0 + 0.1f, ya, -L * t0, x1 - 0.1f, ya, -L * t0, x1 - 0.1f, yb, -L * t1, x0 + 0.1f, yb, -L * t1, 0xFACC15);
        }
        mb.quad(x0, 0.02f + e, 0f, x0 + 0.12f, 0.02f + e, 0f, x0 + 0.12f, H + e, -L, x0, H + e, -L, 0x111111);
        mb.quad(x1 - 0.12f, 0.02f + e, 0f, x1, 0.02f + e, 0f, x1, H + e, -L, x1 - 0.12f, H + e, -L, 0x111111);
        ramp = mb.build(g);
    }

    private void buildPlane(Gfx g, MeshBuilder mb) {
        int A = MeshBuilder.ALL;
        mb.reset();
        mb.box(-0.45f, -0.45f, -2.2f, 0.45f, 0.45f, 2.2f, 0xF8FAFC, A);
        mb.box(-0.36f, -0.36f, -2.8f, 0.36f, 0.3f, -2.2f, 0xF8FAFC, A);
        mb.box(-0.3f, 0.08f, -2.78f, 0.3f, 0.31f, -2.3f, 0x1E3A8A, A);
        mb.box(-2.9f, -0.14f, -0.6f, 2.9f, 0f, 0.65f, 0xF1F5F9, A);
        mb.box(-2.9f, -0.15f, -0.6f, -2.5f, 0.01f, 0.65f, 0x009C3B, A);
        mb.box(2.5f, -0.15f, -0.6f, 2.9f, 0.01f, 0.65f, 0x009C3B, A);
        mb.box(-1.45f, -0.5f, -0.7f, -1.0f, -0.14f, 0.3f, 0x94A3B8, A);
        mb.box(1.0f, -0.5f, -0.7f, 1.45f, -0.14f, 0.3f, 0x94A3B8, A);
        mb.box(-0.47f, -0.12f, -2.2f, 0.47f, 0.0f, 2.2f, 0x009C3B, A);
        mb.box(-0.47f, 0.0f, -2.2f, 0.47f, 0.07f, 2.2f, 0xFFDF00, A);
        mb.box(-0.06f, 0.45f, 1.75f, 0.06f, 1.0f, 2.3f, 0x009C3B, A);
        mb.box(-1.15f, 0f, 1.7f, 1.15f, 0.09f, 2.3f, 0xF1F5F9, A);
        for (int i = 0; i < 6; i++) {
            float z = -1.7f + i * 0.6f;
            mb.box(-0.47f, 0.12f, z, 0.47f, 0.3f, z + 0.3f, 0x1E3A8A, A);
        }
        plane = mb.build(g);
    }

    private void buildDuck(Gfx g, MeshBuilder mb) {
        int A = MeshBuilder.ALL;
        int Y = 0xFFD21F, O = 0xFF8C1A;
        mb.reset();
        mb.boxAO(-0.8f, 0.45f, -0.9f, 0.8f, 1.65f, 1.1f, Y, A, 0.85f);
        mb.box(-0.7f, 0.55f, -1.05f, 0.7f, 1.45f, -0.9f, Y, A);
        mb.box(-0.4f, 1.35f, 0.95f, 0.4f, 1.95f, 1.35f, Y, A);
        mb.box(-0.55f, 1.55f, -1.1f, 0.55f, 2.6f, -0.05f, Y, A);
        mb.box(-0.45f, 1.78f, -1.65f, 0.45f, 2.06f, -1.1f, O, A);
        mb.box(-0.38f, 1.63f, -1.5f, 0.38f, 1.79f, -1.1f, 0xE06D00, A);
        for (int s = -1; s <= 1; s += 2) {
            float a = s * 0.55f, b = s * 0.585f, c = s * 0.6f;
            mb.box(Math.min(a, b), 2.0f, -0.88f, Math.max(a, b), 2.36f, -0.48f, 0xFFFFFF, A);
            mb.box(Math.min(b, c), 2.07f, -0.82f, Math.max(b, c), 2.29f, -0.6f, 0x111111, A);
            // olhos de frente tambem (aparece na tela de derrota)
            mb.box(s * 0.36f - 0.11f, 2.12f, -1.115f, s * 0.36f + 0.11f, 2.4f, -1.1f, 0xFFFFFF, A);
            mb.box(s * 0.34f - 0.06f, 2.17f, -1.125f, s * 0.34f + 0.06f, 2.33f, -1.115f, 0x111111, A);
        }
        duckBody = mb.build(g);

        mb.reset();
        mb.box(0f, -0.3f, -0.65f, 0.15f, 0.3f, 0.65f, Y, A);
        duckWing = mb.build(g);

        mb.reset();
        mb.box(-0.07f, -0.5f, -0.07f, 0.07f, 0f, 0.07f, O, A);
        mb.box(-0.22f, -0.58f, -0.38f, 0.22f, -0.5f, 0.1f, O, A);
        duckLeg = mb.build(g);
    }

    private void buildLula(Gfx g, MeshBuilder mb, int si) {
        int A = MeshBuilder.ALL;
        int[] c = SKINS[si];
        int jacket = c[0], pants = c[1], shirt = c[2], tie = c[3], shoes = c[4];

        mb.reset();
        mb.box(-0.11f, -0.74f, -0.12f, 0.11f, 0.02f, 0.12f, pants, A);
        mb.box(-0.12f, -0.86f, -0.21f, 0.12f, -0.72f, 0.13f, shoes, A);
        leg[si] = mb.build(g);

        mb.reset();
        mb.box(-0.085f, -0.56f, -0.095f, 0.085f, 0.05f, 0.095f, jacket, A);
        mb.box(-0.075f, -0.6f, -0.085f, 0.075f, -0.56f, 0.085f, shirt, A);
        mb.box(-0.07f, -0.75f, -0.075f, 0.07f, -0.6f, 0.075f, SKIN, A);
        arm[si] = mb.build(g);

        mb.reset();
        mb.box(-0.25f, -0.08f, -0.15f, 0.25f, 0.06f, 0.15f, pants, A);
        mb.boxAO(-0.3f, 0.02f, -0.17f, 0.3f, 0.64f, 0.17f, jacket, A, 0.9f);
        mb.box(-0.24f, 0.08f, -0.215f, 0.24f, 0.40f, -0.17f, jacket, A);
        mb.box(-0.1f, 0.40f, -0.185f, 0.1f, 0.63f, -0.17f, shirt, A);
        mb.box(-0.036f, 0.40f, -0.197f, 0.036f, 0.61f, -0.18f, tie, A);
        mb.box(-0.04f, 0.22f, -0.228f, 0.04f, 0.41f, -0.21f, tie, A);
        mb.box(-0.13f, 0.62f, -0.13f, 0.13f, 0.68f, 0.13f, shirt, A);
        mb.box(-0.08f, 0.64f, -0.08f, 0.08f, 0.74f, 0.08f, SKIN, A);
        if (si == 3) {
            // listras do timao
            mb.box(-0.305f, 0.5f, -0.175f, 0.305f, 0.55f, 0.175f, 0x18181B, A);
        }
        torso[si] = mb.build(g);

        mb.reset();
        int nose = 0xC98D55;
        mb.box(-0.21f, 0f, -0.2f, 0.21f, 0.46f, 0.2f, SKIN, A);
        mb.box(-0.225f, 0.42f, -0.17f, 0.225f, 0.5f, 0.225f, HAIR, A);
        mb.box(-0.225f, 0.12f, 0.17f, 0.225f, 0.44f, 0.225f, HAIR, A);
        mb.box(-0.225f, 0.22f, -0.05f, -0.205f, 0.44f, 0.2f, HAIR, A);
        mb.box(0.205f, 0.22f, -0.05f, 0.225f, 0.44f, 0.2f, HAIR, A);
        mb.box(-0.215f, -0.04f, -0.24f, 0.215f, 0.17f, -0.15f, BEARD, A);
        mb.box(-0.226f, -0.02f, -0.2f, -0.18f, 0.3f, 0.07f, BEARD, A);
        mb.box(0.18f, -0.02f, -0.2f, 0.226f, 0.3f, 0.07f, BEARD, A);
        mb.box(-0.2f, -0.07f, -0.22f, 0.2f, 0.0f, 0.06f, BEARD, A);
        mb.box(-0.13f, 0.15f, -0.255f, 0.13f, 0.205f, -0.2f, 0xC4C4C4, A);
        mb.box(-0.06f, 0.1f, -0.246f, 0.06f, 0.125f, -0.24f, 0x7A2E2E, A);
        mb.box(-0.045f, 0.18f, -0.255f, 0.045f, 0.31f, -0.2f, nose, A);
        for (int s = -1; s <= 1; s += 2) {
            float xa = s * 0.055f, xb = s * 0.14f;
            mb.box(Math.min(xa, xb), 0.285f, -0.208f, Math.max(xa, xb), 0.335f, -0.2f, 0xFFFFFF, A);
            float pa = s * 0.075f, pb = s * 0.115f;
            mb.box(Math.min(pa, pb), 0.29f, -0.213f, Math.max(pa, pb), 0.33f, -0.205f, 0x2B1B10, A);
            float ea = s * 0.045f, eb = s * 0.155f;
            mb.box(Math.min(ea, eb), 0.36f, -0.218f, Math.max(ea, eb), 0.395f, -0.19f, 0xB5B5B5, A);
            float ka = s * 0.21f, kb = s * 0.237f;
            mb.box(Math.min(ka, kb), 0.2f, -0.03f, Math.max(ka, kb), 0.32f, 0.06f, nose, A);
        }
        head[si] = mb.build(g);
    }

    private void buildSash(Gfx g, MeshBuilder mb) {
        float[] m = new float[16];
        Mat.identity(m);
        Mat.translate(m, 0f, 0.34f, 0f);
        Mat.rotZ(m, 0.72f);
        mb.reset();
        mb.setTransform(m);
        mb.box(-0.085f, -0.42f, -0.225f, 0.085f, 0.42f, 0.195f, 0x009C3B, MeshBuilder.ALL);
        mb.box(-0.03f, -0.42f, -0.232f, 0.03f, 0.42f, 0.202f, 0xFFDF00, MeshBuilder.ALL);
        mb.clearTransform();
        sash = mb.build(g);
    }

    /** Congresso Nacional ao fundo + nuvens. */
    private void buildBackdrop(Gfx g, MeshBuilder mb) {
        int V = MeshBuilder.VIS;
        int W = 0xEDEAE3;
        mb.reset();
        // nuvens
        java.util.Random cr = new java.util.Random(7);
        for (int i = 0; i < 14; i++) {
            float x = -260f + i * 40f + cr.nextFloat() * 20f, y = 55f + cr.nextFloat() * 50f, z = -40f - cr.nextFloat() * 60f;
            float w = 18f + cr.nextFloat() * 22f;
            int CF = MeshBuilder.BOT | MeshBuilder.FRONT | MeshBuilder.LEFT | MeshBuilder.RIGHT;
            mb.box(x - w, y, z - 6f, x + w, y + 4f, z, 0xFFFFFF, CF);
            mb.box(x - w * 0.55f, y + 4f, z - 5f, x + w * 0.45f, y + 8f, z - 1f, 0xFFFFFF, CF);
        }
        mb.boxAO(-80f, -8f, -30f, 80f, 7f, 0f, W, V, 0.8f);
        mb.box(-80f, 7f, -30f, 80f, 7.6f, 0.5f, 0xF6F4EF, V);
        mb.boxAO(-9f, 7f, -18f, -1.6f, 70f, -12f, W, V, 0.85f);
        mb.boxAO(1.6f, 7f, -18f, 9f, 70f, -12f, W, V, 0.85f);
        for (int i = 0; i < 3; i++) {
            float y = 30f + i * 12f;
            mb.box(-1.6f, y, -17f, 1.6f, y + 2.5f, -13f, 0xD9D5CC, V);
        }
        for (int k = 0; k < 12; k++) {
            float y = 10f + k * 5f;
            mb.box(-9.05f, y, -12f, -1.55f, y + 0.6f, -11.95f, 0xB9C3C9, V);
            mb.box(1.55f, y, -12f, 9.05f, y + 0.6f, -11.95f, 0xB9C3C9, V);
        }
        // camara: cupula virada para cima (tigela)
        int N = 18;
        float cx = 38f, cz = -14f;
        for (int i = 0; i < N; i++) {
            double a0 = Math.PI * 2 * i / N, a1 = Math.PI * 2 * (i + 1) / N;
            float c0 = (float) Math.cos(a0), s0 = (float) Math.sin(a0), c1 = (float) Math.cos(a1), s1 = (float) Math.sin(a1);
            float rb = 6f, rt = 16f, yb = 7.6f, yt = 14.5f;
            mb.quad(cx + c0 * rb, yb, cz + s0 * rb, cx + c0 * rt, yt, cz + s0 * rt,
                    cx + c1 * rt, yt, cz + s1 * rt, cx + c1 * rb, yb, cz + s1 * rb, 0xF2F0EA);
        }
        // senado: cupula convexa
        float sx = -38f;
        int RINGS = 4;
        for (int r = 0; r < RINGS; r++) {
            double p0 = Math.PI * 0.5 * r / RINGS, p1 = Math.PI * 0.5 * (r + 1) / RINGS;
            float r0 = 11f * (float) Math.cos(p0), r1 = 11f * (float) Math.cos(p1);
            float y0 = 7.6f + 6.5f * (float) Math.sin(p0), y1 = 7.6f + 6.5f * (float) Math.sin(p1);
            for (int i = 0; i < N; i++) {
                double a0 = Math.PI * 2 * i / N, a1 = Math.PI * 2 * (i + 1) / N;
                float c0 = (float) Math.cos(a0), s0 = (float) Math.sin(a0), c1 = (float) Math.cos(a1), s1 = (float) Math.sin(a1);
                mb.quad(sx + c0 * r0, y0, cz + s0 * r0, sx + c0 * r1, y1, cz + s0 * r1,
                        sx + c1 * r1, y1, cz + s1 * r1, sx + c1 * r0, y0, cz + s1 * r0, 0xF2F0EA);
            }
        }
        backdrop = mb.build(g);
    }
}
