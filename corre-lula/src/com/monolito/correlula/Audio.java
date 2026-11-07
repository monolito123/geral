package com.monolito.correlula;

import android.media.AudioFormat;
import android.media.AudioManager;
import android.media.AudioTrack;
import android.util.Log;

/** Mixer de audio proprio: uma unica AudioTrack em streaming, sons sintetizados em memoria. */
final class Audio implements Game.Sound, Runnable {
    private static final int NV = 10;
    private short[][] fx;
    private short[] music;
    private final short[][] vData = new short[NV][];
    private final float[] vPos = new float[NV], vRate = new float[NV], vVol = new float[NV];
    private final int[] rId = new int[32];
    private final float[] rVol = new float[32], rRate = new float[32];
    private int rN;
    private float musicPos;
    private volatile boolean running = true, paused, muted, musicOn = true, ready;
    private final Thread thread;

    Audio() {
        thread = new Thread(this, "audio");
        thread.start();
    }

    @Override
    public synchronized void play(int id, float vol, float rate) {
        if (muted || !ready || rN >= rId.length) return;
        rId[rN] = id;
        rVol[rN] = vol;
        rRate[rN] = rate;
        rN++;
    }

    @Override
    public synchronized void setMuted(boolean m) {
        muted = m;
        notifyAll();
    }

    @Override
    public void setMusic(boolean on) {
        musicOn = on;
    }

    synchronized void pause() {
        paused = true;
    }

    synchronized void resume() {
        paused = false;
        notifyAll();
    }

    synchronized void shutdown() {
        running = false;
        notifyAll();
    }

    @Override
    public void run() {
        AudioTrack track = null;
        try {
            fx = Synth.effects();
            music = Synth.music();
            int rate = Synth.RATE;
            int min = AudioTrack.getMinBufferSize(rate, AudioFormat.CHANNEL_OUT_MONO, AudioFormat.ENCODING_PCM_16BIT);
            int bufBytes = Math.max(min, 4096);
            track = new AudioTrack(AudioManager.STREAM_MUSIC, rate, AudioFormat.CHANNEL_OUT_MONO,
                    AudioFormat.ENCODING_PCM_16BIT, bufBytes, AudioTrack.MODE_STREAM);
            if (track.getState() != AudioTrack.STATE_INITIALIZED) {
                track.release();
                return;
            }
            ready = true;
            final int N = 256;
            short[] out = new short[N];
            float[] mix = new float[N];
            boolean playing = false;
            while (running) {
                synchronized (this) {
                    while (running && (paused || muted)) {
                        if (playing) { track.pause(); track.flush(); playing = false; }
                        rN = 0;
                        try { wait(); } catch (InterruptedException e) { return; }
                    }
                    for (int i = 0; i < rN; i++) start(rId[i], rVol[i], rRate[i]);
                    rN = 0;
                }
                if (!running) break;
                if (!playing) { track.play(); playing = true; }
                java.util.Arrays.fill(mix, 0f);
                if (musicOn && music != null) {
                    int ml = music.length;
                    for (int i = 0; i < N; i++) {
                        mix[i] += music[(int) musicPos] * 0.5f;
                        musicPos += 1f;
                        if (musicPos >= ml) musicPos -= ml;
                    }
                }
                for (int v = 0; v < NV; v++) {
                    short[] d = vData[v];
                    if (d == null) continue;
                    float p = vPos[v], r = vRate[v], vol = vVol[v];
                    int len = d.length;
                    for (int i = 0; i < N; i++) {
                        int k = (int) p;
                        if (k >= len) { d = null; break; }
                        mix[i] += d[k] * vol;
                        p += r;
                    }
                    vPos[v] = p;
                    if (d == null) vData[v] = null;
                }
                for (int i = 0; i < N; i++) {
                    float s = mix[i];
                    if (s > 32767f) s = 32767f;
                    else if (s < -32768f) s = -32768f;
                    out[i] = (short) s;
                }
                track.write(out, 0, N);
            }
        } catch (Throwable t) {
            Log.w("CorreLula", "audio desligado: " + t);
        } finally {
            ready = false;
            if (track != null) {
                try { track.stop(); } catch (Throwable ignored) { }
                track.release();
            }
        }
    }

    private void start(int id, float vol, float rate) {
        if (fx == null || id < 0 || id >= fx.length) return;
        int slot = -1;
        float best = -1;
        for (int v = 0; v < NV; v++) {
            if (vData[v] == null) { slot = v; break; }
            float prog = vPos[v] / vData[v].length;
            if (prog > best) { best = prog; slot = v; }
        }
        vData[slot] = fx[id];
        vPos[slot] = 0;
        vRate[slot] = rate;
        vVol[slot] = vol;
    }
}
