package com.monolito.correlula;

import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Typeface;

/** Desenha as letras do atlas com o Canvas do Android. */
final class AndroidPainter implements Font.Painter {
    private Paint fill, stroke;
    private Bitmap bmp;
    private Canvas cv;
    private final char[] one = new char[1];

    @Override
    public void setup(float px, float strokeW) {
        fill = new Paint(Paint.ANTI_ALIAS_FLAG);
        fill.setTypeface(Typeface.create(Typeface.SANS_SERIF, Typeface.BOLD));
        fill.setTextSize(px);
        fill.setColor(Color.WHITE);
        stroke = new Paint(fill);
        stroke.setStyle(Paint.Style.STROKE);
        stroke.setStrokeWidth(strokeW);
        stroke.setStrokeJoin(Paint.Join.ROUND);
        stroke.setStrokeCap(Paint.Cap.ROUND);
        stroke.setColor(Color.BLACK);
    }

    @Override
    public float ascent() {
        return -fill.getFontMetrics().ascent;
    }

    @Override
    public float descent() {
        return fill.getFontMetrics().descent;
    }

    @Override
    public float advance(char c) {
        one[0] = c;
        return fill.measureText(one, 0, 1);
    }

    @Override
    public void begin(int w, int h) {
        bmp = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888);
        cv = new Canvas(bmp);
    }

    @Override
    public void draw(char c, float x, float baseline) {
        if (c == ' ') return;
        one[0] = c;
        cv.drawText(one, 0, 1, x, baseline, stroke);
        cv.drawText(one, 0, 1, x, baseline, fill);
    }

    @Override
    public int[] finish() {
        int w = bmp.getWidth(), h = bmp.getHeight();
        int[] px = new int[w * h];
        bmp.getPixels(px, 0, w, 0, 0, w, h);
        bmp.recycle();
        bmp = null;
        cv = null;
        return px;
    }
}
