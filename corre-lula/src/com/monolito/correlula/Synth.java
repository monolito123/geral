package com.monolito.correlula;

import java.util.Random;

/** Efeitos e musica gerados por codigo (PCM 16 bits mono, 22050 Hz). Nada de arquivos de audio. */
final class Synth {
    static final int RATE = 22050;
    static final int COIN = 0, JUMP = 1, SLIDE = 2, LANE = 3, LAND = 4, STUMBLE = 5, CRASH = 6, POWER = 7,
            QUACK = 8, MILESTONE = 9, CLICK = 10, GAMEOVER = 11, BUY = 12, SUPERJUMP = 13, COUNT = 14;

    private Synth() {}

    static short[][] effects() {
        short[][] s = new short[COUNT][];
        s[COIN] = coin();
        s[JUMP] = sweep(260, 640, 0.15f, 0.45f, 1);
        s[SUPERJUMP] = sweep(200, 1100, 0.32f, 0.45f, 1);
        s[SLIDE] = noise(0.22f, 0.32f, 0.92f, 0.6f);
        s[LANE] = noise(0.09f, 0.18f, 0.85f, 0.8f);
        s[LAND] = sweep(140, 70, 0.09f, 0.5f, 0);
        s[STUMBLE] = mix(sweep(190, 80, 0.18f, 0.7f, 0), noise(0.08f, 0.35f, 0.7f, 0.9f));
        s[CRASH] = mix(noise(0.55f, 0.75f, 0.6f, 0.97f), sweep(110, 40, 0.5f, 0.8f, 0));
        s[POWER] = notes(new float[]{72, 76, 79, 84, 88}, 0.07f, 0.32f, 2);
        s[QUACK] = quack();
        s[MILESTONE] = notes(new float[]{79, 83, 86, 91}, 0.09f, 0.3f, 2);
        s[CLICK] = sweep(1800, 1400, 0.03f, 0.3f, 1);
        s[GAMEOVER] = sadTrombone();
        s[BUY] = notes(new float[]{84, 88, 91, 96}, 0.06f, 0.3f, 1);
        return s;
    }

    private static float midi(float n) {
        return 440f * (float) Math.pow(2, (n - 69) / 12.0);
    }

    private static short[] toPcm(float[] f) {
        short[] s = new short[f.length];
        for (int i = 0; i < f.length; i++) {
            float v = f[i];
            if (v > 1f) v = 1f;
            if (v < -1f) v = -1f;
            s[i] = (short) (v * 32000f);
        }
        return s;
    }

    private static short[] mix(short[] a, short[] b) {
        short[] o = new short[Math.max(a.length, b.length)];
        for (int i = 0; i < o.length; i++) {
            int v = (i < a.length ? a[i] : 0) + (i < b.length ? b[i] : 0);
            o[i] = (short) Math.max(-32000, Math.min(32000, v));
        }
        return o;
    }

    private static float wave(int type, float ph) {
        ph -= (float) Math.floor(ph);
        switch (type) {
            case 0: return (float) Math.sin(ph * Math.PI * 2);
            case 1: return ph < 0.5f ? 4 * ph - 1 : 3 - 4 * ph; // triangulo
            case 2: return ph < 0.25f ? 0.8f : -0.8f; // pulso
            default: return ph * 2 - 1; // serra
        }
    }

    private static short[] coin() {
        int n = (int) (RATE * 0.17f);
        float[] o = new float[n];
        float ph = 0;
        for (int i = 0; i < n; i++) {
            float t = i / (float) RATE;
            float f = t < 0.045f ? 1318.5f : 1975.5f;
            ph += f / RATE;
            float env = t < 0.045f ? 1f : (float) Math.exp(-(t - 0.045f) * 22);
            o[i] = (wave(1, ph) * 0.7f + wave(0, ph * 2) * 0.2f) * env * 0.42f;
        }
        return toPcm(o);
    }

    private static short[] sweep(float f0, float f1, float dur, float vol, int type) {
        int n = (int) (RATE * dur);
        float[] o = new float[n];
        float ph = 0;
        for (int i = 0; i < n; i++) {
            float t = i / (float) n;
            float f = f0 + (f1 - f0) * t;
            ph += f / RATE;
            float env = Math.min(1f, i / 80f) * (1 - t) * (1 - t);
            o[i] = wave(type, ph) * env * vol;
        }
        return toPcm(o);
    }

    private static short[] noise(float dur, float vol, float lp, float decay) {
        int n = (int) (RATE * dur);
        float[] o = new float[n];
        Random r = new Random(3);
        float y = 0;
        for (int i = 0; i < n; i++) {
            float t = i / (float) n;
            float a = lp * (1 - t * decay * 0.6f);
            y = y * a + (r.nextFloat() * 2 - 1) * (1 - a);
            float env = Math.min(1f, i / 60f) * (float) Math.pow(1 - t, 1.5);
            o[i] = y * env * vol * 3f;
        }
        return toPcm(o);
    }

    private static short[] notes(float[] ns, float each, float vol, int type) {
        int per = (int) (RATE * each);
        int n = per * ns.length + (int) (RATE * 0.15f);
        float[] o = new float[n];
        for (int k = 0; k < ns.length; k++) {
            float f = midi(ns[k]);
            float ph = 0;
            int st = k * per;
            int len = per + (k == ns.length - 1 ? (int) (RATE * 0.15f) : per / 2);
            for (int i = 0; i < len && st + i < n; i++) {
                ph += f / RATE;
                float env = Math.min(1f, i / 40f) * (float) Math.exp(-i / (float) (RATE * 0.08f));
                o[st + i] += wave(type, ph) * env * vol;
            }
        }
        return toPcm(o);
    }

    private static short[] quack() {
        int one = (int) (RATE * 0.13f), gap = (int) (RATE * 0.05f);
        float[] o = new float[one * 2 + gap];
        for (int q = 0; q < 2; q++) {
            float ph = 0, b1 = 0, b2 = 0;
            int st = q * (one + gap);
            for (int i = 0; i < one; i++) {
                float t = i / (float) one;
                float f = (q == 0 ? 470 : 430) - 170 * t;
                ph += f / RATE;
                float x = wave(3, ph);
                // filtro ressonante simples ~1100 Hz
                float fc = 2f * (float) Math.sin(Math.PI * 1100 / RATE);
                b1 += fc * (x - b1 - 0.35f * b2);
                b2 += fc * b1;
                float env = Math.min(1f, i / 120f) * (1 - t);
                o[st + i] = (b1 * 0.9f + x * 0.15f) * env * 0.5f;
            }
        }
        return toPcm(o);
    }

    private static short[] sadTrombone() {
        float[] ns = {70, 69, 68, 67};
        float[] du = {0.3f, 0.3f, 0.3f, 0.9f};
        int n = 0;
        for (float d : du) n += (int) (RATE * d);
        float[] o = new float[n];
        int st = 0;
        float lp = 0;
        for (int k = 0; k < ns.length; k++) {
            int len = (int) (RATE * du[k]);
            float ph = 0;
            for (int i = 0; i < len; i++) {
                float t = i / (float) RATE;
                float vib = k == 3 ? (float) Math.sin(t * 2 * Math.PI * 6) * 0.35f : 0f;
                float f = midi(ns[k] + vib);
                ph += f / RATE;
                float x = wave(3, ph);
                lp += (x - lp) * 0.18f;
                float env = Math.min(1f, i / 300f) * (k == 3 ? (1 - i / (float) len) : (i < len - 400 ? 1f : (len - i) / 400f));
                o[st + i] = lp * env * 0.45f;
            }
            st += len;
        }
        return toPcm(o);
    }

    // ------------------------------------------------------------------ musica
    /** Baiao original em Sol mixolidio, 8 compassos. */
    static short[] music() {
        float bpm = 128f;
        float beat = 60f / bpm;
        int bars = 8;
        int n = (int) (RATE * beat * 4 * bars);
        float[] o = new float[n];
        // melodia: pares (nota midi, duracao em tempos); 0 = pausa
        float[] mel = {
                74, .5f, 74, .25f, 76, .25f, 77, .5f, 76, .5f, 74, .5f, 71, .5f, 67, 1,
                69, .5f, 71, .5f, 72, .5f, 71, .5f, 69, 1, 67, 1,
                77, .5f, 77, .25f, 76, .25f, 74, .5f, 72, .5f, 69, .5f, 72, .5f, 77, 1,
                74, .75f, 71, .25f, 67, .5f, 71, .5f, 74, 2,
                76, .5f, 76, .25f, 77, .25f, 79, .5f, 76, .5f, 72, 1, 76, 1,
                74, .5f, 76, .5f, 77, .5f, 76, .5f, 74, .5f, 72, .5f, 76, 1,
                74, .5f, 77, .5f, 81, .5f, 77, .5f, 76, .5f, 74, .5f, 72, 1,
                71, .5f, 74, .5f, 79, 1, 77, .5f, 74, .5f, 67, 1,
        };
        // acordes por compasso (fundamental midi do baixo)
        float[] root = {43, 43, 41, 43, 48, 48, 50, 43};
        float[][] chord = {{67, 71, 74}, {67, 71, 74}, {65, 69, 72}, {67, 71, 74}, {64, 67, 72}, {64, 67, 72}, {62, 65, 69}, {67, 71, 74}};

        // melodia (sanfona: pulso + leve vibrato)
        float t = 0;
        for (int i = 0; i < mel.length; i += 2) {
            float note = mel[i], d = mel[i + 1] * beat;
            if (note > 0) tone(o, t, d * 0.92f, midi(note), 0.17f, 2, true);
            t += d;
        }
        for (int b = 0; b < bars; b++) {
            float b0 = b * 4 * beat;
            float r = root[b];
            // baixo do baiao: (1) . . (a) | (2) ...
            for (int half = 0; half < 2; half++) {
                float h0 = b0 + half * 2 * beat;
                tone(o, h0, beat * 0.7f, midi(r), 0.30f, 1, false);
                tone(o, h0 + beat * 0.75f, beat * 0.22f, midi(r + 7), 0.26f, 1, false);
                tone(o, h0 + beat, beat * 0.9f, midi(r + 12), 0.24f, 1, false);
                // zabumba
                kick(o, h0);
                kick(o, h0 + beat * 0.75f);
                rim(o, h0 + beat * 1.5f);
            }
            // acordes nos contratempos
            for (int k = 0; k < 4; k++) {
                float ct = b0 + (k + 0.5f) * beat;
                for (float cn : chord[b]) tone(o, ct, beat * 0.18f, midi(cn - 12), 0.05f, 1, false);
            }
            // triangulo em semicolcheias
            for (int k = 0; k < 16; k++) {
                float tt = b0 + k * beat * 0.25f;
                boolean open = (k % 4) == 2;
                triangle(o, tt, open ? 0.12f : 0.04f, k % 2 == 1 ? 0.09f : 0.05f);
            }
        }
        // normaliza
        float pk = 0;
        for (float v : o) pk = Math.max(pk, Math.abs(v));
        float g = pk > 0 ? 0.55f / pk : 1f;
        for (int i = 0; i < n; i++) o[i] *= g;
        return toPcm(o);
    }

    private static void tone(float[] o, float t0, float dur, float f, float vol, int type, boolean vib) {
        int st = (int) (t0 * RATE), len = (int) (dur * RATE);
        float ph = 0;
        int rel = Math.min(len / 3, 400);
        for (int i = 0; i < len; i++) {
            int k = st + i;
            if (k >= o.length) k -= o.length;
            float tt = i / (float) RATE;
            float ff = vib ? f * (1f + 0.006f * (float) Math.sin(tt * 2 * Math.PI * 5.5f)) : f;
            ph += ff / RATE;
            float env = Math.min(1f, i / 90f);
            if (i > len - rel) env *= (len - i) / (float) rel;
            if (!vib) env *= (float) Math.exp(-tt * 3.5f);
            o[k] += wave(type, ph) * env * vol;
        }
    }

    private static final Random NR = new Random(11);

    private static void kick(float[] o, float t0) {
        int st = (int) (t0 * RATE), len = (int) (0.16f * RATE);
        float ph = 0;
        for (int i = 0; i < len; i++) {
            int k = st + i;
            if (k >= o.length) k -= o.length;
            float tt = i / (float) len;
            ph += (95f - 50f * tt) / RATE;
            o[k] += (float) Math.sin(ph * 2 * Math.PI) * (1 - tt) * (1 - tt) * 0.5f;
        }
    }

    private static void rim(float[] o, float t0) {
        int st = (int) (t0 * RATE), len = (int) (0.05f * RATE);
        float prev = 0;
        for (int i = 0; i < len; i++) {
            int k = st + i;
            if (k >= o.length) k -= o.length;
            float x = NR.nextFloat() * 2 - 1;
            float hp = x - prev;
            prev = x;
            o[k] += hp * (1 - i / (float) len) * 0.12f;
        }
    }

    private static void triangle(float[] o, float t0, float dur, float vol) {
        int st = (int) (t0 * RATE), len = (int) (dur * RATE);
        for (int i = 0; i < len; i++) {
            int k = st + i;
            if (k >= o.length) k -= o.length;
            float tt = i / (float) RATE;
            float v = (float) (Math.sin(tt * 2 * Math.PI * 3950) * 0.6 + Math.sin(tt * 2 * Math.PI * 5270) * 0.4);
            o[k] += v * (float) Math.exp(-tt * (dur > 0.1f ? 18 : 60)) * vol;
        }
    }
}
