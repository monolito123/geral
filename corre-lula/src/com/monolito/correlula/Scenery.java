package com.monolito.correlula;

import java.util.Random;

/** Cenario da Esplanada gerado em pedacos de 30 m (deterministico pelo indice). */
final class Scenery {
    static final float CH = 30f;
    static final int SLOTS = 9;
    static final float BEHIND = 35f, AHEAD = 175f;

    static final class Chunk {
        int idx = Integer.MIN_VALUE;
        Gfx.Mesh ground, props;
        int nt;
        final float[] tx = new float[4], ty = new float[4], ts = new float[4], tw = new float[4], th = new float[4];
        final int[] tcol = new int[4];
        final String[] tstr = new String[4];
    }

    final Chunk[] slots = new Chunk[SLOTS];
    long seed = 1;

    Scenery() {
        for (int i = 0; i < SLOTS; i++) slots[i] = new Chunk();
    }

    static int first(double absP) {
        return (int) Math.floor((absP - BEHIND) / CH);
    }

    static int last(double absP) {
        return (int) Math.floor((absP + AHEAD) / CH);
    }

    Chunk get(int idx) {
        return slots[((idx % SLOTS) + SLOTS) % SLOTS];
    }

    void update(Gfx g, MeshBuilder mb, double absP) {
        int a = first(absP), b = last(absP);
        for (int i = a; i <= b; i++) {
            Chunk c = get(i);
            if (c.idx != i) build(c, i, g, mb);
        }
    }

    /** Recria tudo (troca de partida ou perda do contexto GL). */
    void invalidate(Gfx g, boolean contextLost) {
        for (Chunk c : slots) {
            if (!contextLost && g != null) {
                if (c.ground != null) g.deleteMesh(c.ground);
                if (c.props != null) g.deleteMesh(c.props);
            }
            c.ground = null;
            c.props = null;
            c.idx = Integer.MIN_VALUE;
        }
    }

    private static final int[] IPE = {0xE85A9A, 0xF5C518, 0x9B59B6, 0xF2F2F2, 0x3F8F3A, 0x4E9A3F, 0xE85A9A, 0xF5C518};
    private static final int[] PASTEL = {0xD9C6A5, 0xB8C9D9, 0xE3D3B8, 0xCFD8C2, 0xE8C9C0};
    private static final int[] TEXT_COL = {0xFFC1121F, 0xFF0B6B2E, 0xFF1D4ED8, 0xFF111111, 0xFFB45309};
    private static final int[] FRAME_COL = {0xC1121F, 0x0B6B2E, 0x1D4ED8, 0xF2B705, 0x111827};

    private void build(Chunk c, int idx, Gfx g, MeshBuilder mb) {
        if (c.ground != null) g.deleteMesh(c.ground);
        if (c.props != null) g.deleteMesh(c.props);
        c.idx = idx;
        c.nt = 0;
        Random r = new Random(idx * 1000003L ^ seed * 31L);
        final int TOP = MeshBuilder.TOP, VIS = MeshBuilder.VIS;

        // ---- chao (desenhado sem teste de profundidade, em ordem)
        mb.reset();
        int grass = (idx & 1) == 0 ? 0x6DAA3C : 0x66A137;
        mb.box(-80f, -0.1f, -CH, -6.6f, 0f, 0f, grass, TOP);
        mb.box(6.6f, -0.1f, -CH, 80f, 0f, 0f, grass, TOP);
        mb.box(-4.0f, -0.1f, -CH, 4.0f, 0f, 0f, 0x3B3F46, TOP);
        for (float s = 1f; s < CH; s += 6f) {
            for (int k = -1; k <= 1; k += 2) {
                float x = k * World.LANE_W * 0.5f;
                mb.box(x - 0.07f, 0f, -(s + 3f), x + 0.07f, 0f, -s, 0xE5E7EB, TOP);
            }
        }
        mb.box(-3.82f, 0f, -CH, -3.7f, 0f, 0f, 0xF2C94C, TOP);
        mb.box(3.7f, 0f, -CH, 3.82f, 0f, 0f, 0xF2C94C, TOP);
        if (r.nextInt(5) == 0) {
            for (float x = -3.5f; x < 3.4f; x += 0.9f) mb.box(x, 0f, -17.5f, x + 0.5f, 0f, -14f, 0xF1F1F1, TOP);
        }
        c.ground = mb.build(g);

        // ---- objetos
        mb.reset();
        for (int side = -1; side <= 1; side += 2) {
            float sg = side;
            box(mb, sg, 4.0f, 4.24f, 0f, 0.24f, 0f, CH, 0xDADADA, VIS);
            box(mb, sg, 4.24f, 6.6f, 0f, 0.17f, 0f, CH, 0xBDB7AC, VIS);
            // postes de luz
            for (int k = 0; k < 2; k++) {
                float s = 7f + k * 15f;
                box(mb, sg, 4.55f, 4.72f, 0f, 6.3f, s - 0.09f, s + 0.09f, 0x4B5563, VIS);
                box(mb, sg, 3.3f, 4.7f, 6.05f, 6.18f, s - 0.07f, s + 0.07f, 0x4B5563, VIS);
                box(mb, sg, 3.05f, 3.6f, 5.88f, 6.08f, s - 0.18f, s + 0.18f, 0xFDE68A, VIS);
            }
        }
        int bbSide = (idx & 1) == 0 ? -1 : 1;
        boolean hasBoard = r.nextFloat() < 0.88f;
        float bbS = 13f;
        // arvores (ipes)
        for (int side = -1; side <= 1; side += 2) {
            int n = 2 + r.nextInt(3);
            for (int i = 0; i < n; i++) {
                float x = 8.3f + r.nextFloat() * 8.5f;
                float s = 2f + r.nextFloat() * (CH - 4f);
                if (side == bbSide && hasBoard && Math.abs(s - bbS) < 5f && x < 15.5f) continue;
                tree(mb, side, x, s, IPE[r.nextInt(IPE.length)], 0.8f + r.nextFloat() * 0.5f);
            }
        }
        // predios
        for (int side = -1; side <= 1; side += 2) {
            float t = r.nextFloat();
            if (t < 0.5f) ministry(mb, side, 16f + r.nextFloat() * 7f);
            else if (t < 0.85f) pilotis(mb, side, r, 3 + r.nextInt(4));
            else {
                tree(mb, side, 22f, 10f, IPE[r.nextInt(IPE.length)], 1.6f);
                tree(mb, side, 27f, 21f, IPE[r.nextInt(IPE.length)], 1.4f);
            }
        }
        // outdoor com meme
        if (hasBoard) {
            float cx = 10.9f;
            int fc = FRAME_COL[r.nextInt(FRAME_COL.length)];
            box(mb, bbSide, cx - 2.5f, cx - 2.2f, 0f, 4.0f, bbS - 0.12f, bbS + 0.12f, 0x6B7280, VIS);
            box(mb, bbSide, cx + 2.2f, cx + 2.5f, 0f, 4.0f, bbS - 0.12f, bbS + 0.12f, 0x6B7280, VIS);
            box(mb, bbSide, cx - 3.35f, cx + 3.35f, 3.75f, 7.45f, bbS - 0.16f, bbS + 0.16f, fc, VIS);
            box(mb, bbSide, cx - 3.12f, cx + 3.12f, 3.97f, 7.23f, bbS - 0.19f, bbS + 0.1f, 0xFAFAF7, VIS);
            int t = c.nt++;
            c.tx[t] = bbSide * cx;
            c.ty[t] = 5.6f;
            c.ts[t] = bbS - 0.2f;
            c.tw[t] = 5.9f;
            c.th[t] = 2.9f;
            c.tcol[t] = TEXT_COL[r.nextInt(TEXT_COL.length)];
            c.tstr[t] = Memes.BILLBOARDS[r.nextInt(Memes.BILLBOARDS.length)];
        }
        // barraca de churrasco
        if (r.nextFloat() < 0.14f) {
            int side = -bbSide;
            float cx = 8.4f, s = 22f;
            box(mb, side, cx - 1.4f, cx + 1.4f, 0f, 1.0f, s - 0.7f, s + 0.7f, 0x8B5A2B, VIS);
            box(mb, side, cx - 1.1f, cx + 0.2f, 1.0f, 1.18f, s - 0.45f, s + 0.45f, 0x1F1F1F, VIS);
            box(mb, side, cx - 1.0f, cx + 0.1f, 1.18f, 1.22f, s - 0.35f, s + 0.35f, 0xFF5A1F, VIS);
            box(mb, side, cx - 1.45f, cx - 1.35f, 1.0f, 2.25f, s - 0.05f, s + 0.05f, 0x4B5563, VIS);
            box(mb, side, cx + 1.35f, cx + 1.45f, 1.0f, 2.25f, s - 0.05f, s + 0.05f, 0x4B5563, VIS);
            for (int k = 0; k < 5; k++) {
                float x0 = cx - 1.6f + k * 0.64f;
                box(mb, side, x0, x0 + 0.64f, 2.2f, 2.38f, s - 0.85f, s + 0.85f, (k & 1) == 0 ? 0xD62828 : 0xF8FAFC, VIS);
            }
            box(mb, side, cx - 1.5f, cx + 1.5f, 2.42f, 3.25f, s - 0.06f, s + 0.06f, 0xF2B705, VIS);
            int t = c.nt++;
            c.tx[t] = side * cx;
            c.ty[t] = 2.83f;
            c.ts[t] = s - 0.08f;
            c.tw[t] = 2.8f;
            c.th[t] = 0.7f;
            c.tcol[t] = 0xFF7A1D17;
            c.tstr[t] = r.nextBoolean() ? "PICANHA\nDO POVO" : "CHURRASCO\nDA COMPANHEIRADA";
        }
        // bandeira do Brasil
        if (r.nextFloat() < 0.12f) {
            int side = -bbSide;
            float cx = 7.4f, s = 4f;
            box(mb, side, cx - 0.07f, cx + 0.07f, 0f, 8.6f, s - 0.07f, s + 0.07f, 0xD1D5DB, VIS);
            float fx0 = cx + 0.07f, fx1 = cx + 3.0f, fy0 = 6.4f, fy1 = 8.4f;
            float x0 = side > 0 ? fx0 : -fx1, x1 = side > 0 ? fx1 : -fx0;
            float z = -s;
            mb.quad(x0, fy0, z, x1, fy0, z, x1, fy1, z, x0, fy1, z, 0x009C3B);
            float mx = (x0 + x1) * 0.5f, my = (fy0 + fy1) * 0.5f, hw = (x1 - x0) * 0.42f, hh = (fy1 - fy0) * 0.4f;
            mb.quad(mx, my - hh, z + 0.05f, mx + hw, my, z + 0.05f, mx, my + hh, z + 0.05f, mx - hw, my, z + 0.05f, 0xFFDF00);
            int N = 10;
            float rr = hh * 0.62f;
            for (int i = 0; i < N; i++) {
                double a0 = Math.PI * 2 * i / N, a1 = Math.PI * 2 * (i + 1) / N;
                mb.tri(mx, my, z + 0.1f, mx + (float) Math.cos(a0) * rr, my + (float) Math.sin(a0) * rr, z + 0.1f,
                        mx + (float) Math.cos(a1) * rr, my + (float) Math.sin(a1) * rr, z + 0.1f, 0x002776);
            }
        }
        c.props = mb.build(g);
    }

    /** Caixa espelhada pelo lado da pista. x0..x1 sao distancias positivas a partir do centro. */
    private static void box(MeshBuilder mb, float side, float x0, float x1, float y0, float y1, float s0, float s1, int col, int faces) {
        float a = side * x0, b = side * x1;
        mb.box(Math.min(a, b), y0, -s1, Math.max(a, b), y1, -s0, col, faces);
    }

    private static void tree(MeshBuilder mb, int side, float x, float s, int col, float sc) {
        final int V = MeshBuilder.VIS;
        box(mb, side, x - 0.18f * sc, x + 0.18f * sc, 0f, 2.5f * sc, s - 0.18f * sc, s + 0.18f * sc, 0x6B4423, V);
        mb.boxAO(side > 0 ? x - 1.4f * sc : -x - 1.4f * sc, 2.2f * sc, -s - 1.4f * sc,
                side > 0 ? x + 1.4f * sc : -x + 1.4f * sc, 3.9f * sc, -s + 1.4f * sc, col, V, 0.78f);
        int c2 = darker(col);
        box(mb, side, x - 0.9f * sc, x + 0.9f * sc, 3.9f * sc, 4.8f * sc, s - 0.9f * sc, s + 0.9f * sc, c2, V);
    }

    private static int darker(int c) {
        int r = (c >> 16) & 255, g = (c >> 8) & 255, b = c & 255;
        return ((r * 9 / 10) << 16) | ((g * 9 / 10) << 8) | (b * 9 / 10);
    }

    private static void ministry(MeshBuilder mb, int side, float h) {
        final int V = MeshBuilder.VIS;
        float x0 = 19f, x1 = 31f, s0 = 4f, s1 = 26f;
        float a = side * x0, b = side * x1;
        mb.boxAO(Math.min(a, b), 0f, -s1, Math.max(a, b), h, -s0, 0xE9E5DA, V, 0.82f);
        for (float y = 2f; y < h - 1.2f; y += 2.4f) {
            box(mb, side, x0 - 0.15f, x0 + 0.05f, y, y + 1.3f, s0 + 0.4f, s1 - 0.4f, 0x4F8A84, V);
            box(mb, side, x0 + 0.5f, x1 - 0.5f, y, y + 1.3f, s0 - 0.12f, s0 + 0.2f, 0x4F8A84, MeshBuilder.FRONT);
        }
        box(mb, side, x0 + 3f, x0 + 7f, h, h + 1.6f, s0 + 6f, s0 + 12f, 0xCFCAC0, V);
    }

    private static void pilotis(MeshBuilder mb, int side, Random r, int floors) {
        final int V = MeshBuilder.VIS;
        float x0 = 19f, x1 = 30f, s0 = 5f, s1 = 25f;
        int col = PASTEL[r.nextInt(PASTEL.length)];
        float base = 3.2f, top = base + floors * 2.8f;
        for (float s = s0 + 0.5f; s <= s1 - 0.4f; s += 4.9f) {
            box(mb, side, x0 + 0.4f, x0 + 1.0f, 0f, base, s - 0.3f, s + 0.3f, 0xE5E1D8, V);
            box(mb, side, x1 - 1.0f, x1 - 0.4f, 0f, base, s - 0.3f, s + 0.3f, 0xE5E1D8, V);
        }
        float a = side * x0, b = side * x1;
        mb.boxAO(Math.min(a, b), base, -s1, Math.max(a, b), top, -s0, col, V, 0.85f);
        for (int f = 0; f < floors; f++) {
            float y = base + f * 2.8f + 0.9f;
            box(mb, side, x0 - 0.12f, x0 + 0.05f, y, y + 1.2f, s0 + 0.5f, s1 - 0.5f, 0x3E4C59, V);
        }
        mb.box(Math.min(a, b) + 0.2f, top, -s1 + 0.2f, Math.max(a, b) - 0.2f, top + 0.4f, -s0 - 0.2f, 0xF0EEE8, V);
    }
}
