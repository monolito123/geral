package com.monolito.correlula;

import android.app.Activity;
import android.content.Context;
import android.content.SharedPreferences;
import android.media.AudioManager;
import android.opengl.GLSurfaceView;
import android.os.Build;
import android.os.Bundle;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.Window;
import android.view.WindowManager;

import javax.microedition.khronos.egl.EGL10;
import javax.microedition.khronos.egl.EGLConfig;
import javax.microedition.khronos.egl.EGLDisplay;
import javax.microedition.khronos.opengles.GL10;

/** Corre, Lula! - parodia de corredor infinito. Tudo e desenhado e sintetizado por codigo. */
public final class MainActivity extends Activity {
    private GameView view;
    private Audio audio;
    private Game game;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        requestWindowFeature(Window.FEATURE_NO_TITLE);
        getWindow().setFlags(WindowManager.LayoutParams.FLAG_FULLSCREEN | WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON,
                WindowManager.LayoutParams.FLAG_FULLSCREEN | WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        setVolumeControlStream(AudioManager.STREAM_MUSIC);
        audio = new Audio();
        final SharedPreferences sp = getSharedPreferences("corre_lula", Context.MODE_PRIVATE);
        game = new Game(new Game.Store() {
            @Override
            public int get(String k, int def) {
                return sp.getInt(k, def);
            }

            @Override
            public void put(String k, int v) {
                sp.edit().putInt(k, v).apply();
            }
        }, audio);
        game.exitHook = new Runnable() {
            @Override
            public void run() {
                runOnUiThread(new Runnable() {
                    @Override
                    public void run() {
                        finish();
                    }
                });
            }
        };
        view = new GameView(this, game);
        setContentView(view);
        immersive();
    }

    @SuppressWarnings("deprecation")
    private void immersive() {
        if (Build.VERSION.SDK_INT >= 19) {
            getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                    | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_FULLSCREEN
                    | View.SYSTEM_UI_FLAG_LAYOUT_STABLE | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                    | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN);
        }
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) immersive();
    }

    @Override
    protected void onResume() {
        super.onResume();
        view.onResume();
        audio.resume();
        immersive();
    }

    @Override
    protected void onPause() {
        game.pauseFromSystem();
        view.onPause();
        audio.pause();
        super.onPause();
    }

    @Override
    protected void onDestroy() {
        audio.shutdown();
        super.onDestroy();
    }

    @Override
    public boolean onKeyDown(int keyCode, KeyEvent event) {
        switch (keyCode) {
            case KeyEvent.KEYCODE_BACK:
                game.input(Game.T_BACK, 0, 0);
                return true;
            case KeyEvent.KEYCODE_DPAD_LEFT:
            case KeyEvent.KEYCODE_A:
                game.input(Game.T_KEY, World.SW_LEFT, 0);
                return true;
            case KeyEvent.KEYCODE_DPAD_RIGHT:
            case KeyEvent.KEYCODE_D:
                game.input(Game.T_KEY, World.SW_RIGHT, 0);
                return true;
            case KeyEvent.KEYCODE_DPAD_UP:
            case KeyEvent.KEYCODE_W:
                game.input(Game.T_KEY, World.SW_UP, 0);
                return true;
            case KeyEvent.KEYCODE_DPAD_DOWN:
            case KeyEvent.KEYCODE_S:
                game.input(Game.T_KEY, World.SW_DOWN, 0);
                return true;
            case KeyEvent.KEYCODE_ENTER:
            case KeyEvent.KEYCODE_SPACE:
            case KeyEvent.KEYCODE_DPAD_CENTER:
            case KeyEvent.KEYCODE_BUTTON_A:
                game.input(Game.T_KEY, 50, 0);
                return true;
            default:
                return super.onKeyDown(keyCode, event);
        }
    }

    @Override
    public void onBackPressed() {
        game.input(Game.T_BACK, 0, 0);
    }

    /** Superficie OpenGL com o laco do jogo. */
    static final class GameView extends GLSurfaceView implements GLSurfaceView.Renderer {
        private final Game game;
        private long last;
        private int vw = 1, vh = 1;

        GameView(Context c, Game game) {
            super(c);
            this.game = game;
            setEGLContextClientVersion(2);
            setEGLConfigChooser(new ConfigChooser());
            if (Build.VERSION.SDK_INT >= 11) setPreserveEGLContextOnPause(true);
            setRenderer(this);
            setRenderMode(RENDERMODE_CONTINUOUSLY);
            setFocusable(true);
            setFocusableInTouchMode(true);
        }

        @Override
        protected void onSizeChanged(int w, int h, int ow, int oh) {
            super.onSizeChanged(w, h, ow, oh);
            vw = Math.max(1, w);
            vh = Math.max(1, h);
            // limita a resolucao interna (lado menor <= 720 px): economiza GPU e bateria
            int small = Math.min(w, h);
            if (small > 720) {
                float s = 720f / small;
                getHolder().setFixedSize(Math.round(w * s), Math.round(h * s));
            }
        }

        @Override
        public boolean onTouchEvent(MotionEvent e) {
            float x = e.getX() / vw, y = e.getY() / vh;
            switch (e.getActionMasked()) {
                case MotionEvent.ACTION_DOWN:
                    game.input(Game.T_DOWN, x, y);
                    break;
                case MotionEvent.ACTION_MOVE:
                    game.input(Game.T_MOVE, x, y);
                    break;
                case MotionEvent.ACTION_UP:
                case MotionEvent.ACTION_CANCEL:
                    game.input(Game.T_UP, x, y);
                    break;
                default:
                    break;
            }
            return true;
        }

        @Override
        public void onSurfaceCreated(GL10 gl, EGLConfig config) {
            GLBackend b = new GLBackend();
            b.init();
            game.surfaceCreated(b, new AndroidPainter());
            last = System.nanoTime();
        }

        @Override
        public void onSurfaceChanged(GL10 gl, int w, int h) {
            game.surfaceChanged(w, h);
        }

        @Override
        public void onDrawFrame(GL10 gl) {
            long now = System.nanoTime();
            float dt = (now - last) / 1e9f;
            last = now;
            if (dt < 0f) dt = 0f;
            game.frame(dt);
        }
    }

    /** Escolhe um formato de tela leve: sem antialiasing, profundidade 24 se houver (senao 16). */
    static final class ConfigChooser implements GLSurfaceView.EGLConfigChooser {
        private static final int EGL_OPENGL_ES2_BIT = 4;

        @Override
        public EGLConfig chooseConfig(EGL10 egl, EGLDisplay d) {
            int[] attrs = {EGL10.EGL_RED_SIZE, 5, EGL10.EGL_GREEN_SIZE, 6, EGL10.EGL_BLUE_SIZE, 5,
                    EGL10.EGL_DEPTH_SIZE, 16, EGL10.EGL_RENDERABLE_TYPE, EGL_OPENGL_ES2_BIT, EGL10.EGL_NONE};
            int[] num = new int[1];
            egl.eglChooseConfig(d, attrs, null, 0, num);
            if (num[0] <= 0) throw new IllegalArgumentException("Nenhuma configuracao OpenGL ES 2.0");
            EGLConfig[] cfgs = new EGLConfig[num[0]];
            egl.eglChooseConfig(d, attrs, cfgs, num[0], num);
            EGLConfig best = null;
            int bestScore = Integer.MIN_VALUE;
            int[] v = new int[1];
            for (EGLConfig c : cfgs) {
                if (c == null) continue;
                int r = get(egl, d, c, EGL10.EGL_RED_SIZE, v), g = get(egl, d, c, EGL10.EGL_GREEN_SIZE, v);
                int b = get(egl, d, c, EGL10.EGL_BLUE_SIZE, v), a = get(egl, d, c, EGL10.EGL_ALPHA_SIZE, v);
                int dep = get(egl, d, c, EGL10.EGL_DEPTH_SIZE, v), st = get(egl, d, c, EGL10.EGL_STENCIL_SIZE, v);
                int smp = get(egl, d, c, EGL10.EGL_SAMPLES, v);
                int score = 0;
                score += dep >= 24 ? 100 : (dep >= 16 ? 50 : -1000);
                if (r == 8 && g == 8 && b == 8) score += 20;
                else if (r == 5 && g == 6 && b == 5) score += 15;
                if (a == 0) score += 5;
                if (st == 0) score += 2;
                if (smp > 0) score -= 500;
                if (score > bestScore) {
                    bestScore = score;
                    best = c;
                }
            }
            if (best == null) best = cfgs[0];
            return best;
        }

        private static int get(EGL10 egl, EGLDisplay d, EGLConfig c, int attr, int[] v) {
            return egl.eglGetConfigAttrib(d, c, attr, v) ? v[0] : 0;
        }
    }
}
