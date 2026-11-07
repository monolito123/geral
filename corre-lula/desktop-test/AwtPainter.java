package com.monolito.correlula;

import java.awt.BasicStroke;
import java.awt.Color;
import java.awt.Graphics2D;
import java.awt.RenderingHints;
import java.awt.Shape;
import java.awt.font.FontRenderContext;
import java.awt.font.TextLayout;
import java.awt.geom.AffineTransform;
import java.awt.image.BufferedImage;

/** Desenha as letras do atlas com Java2D (equivalente ao AndroidPainter). */
final class AwtPainter implements Font.Painter {
    private java.awt.Font font;
    private float stroke;
    private BufferedImage img;
    private Graphics2D g;
    private final FontRenderContext frc = new FontRenderContext(null, true, true);

    public void setup(float px, float stroke) {
        this.stroke = stroke;
        font = new java.awt.Font("SansSerif", java.awt.Font.BOLD, 1).deriveFont(px);
    }

    public float ascent() {
        return font.getLineMetrics("Hg", frc).getAscent();
    }

    public float descent() {
        return font.getLineMetrics("Hg", frc).getDescent();
    }

    public float advance(char c) {
        return (float) font.getStringBounds(String.valueOf(c), frc).getWidth();
    }

    public void begin(int w, int h) {
        img = new BufferedImage(w, h, BufferedImage.TYPE_INT_ARGB);
        g = img.createGraphics();
        g.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_ON);
        g.setRenderingHint(RenderingHints.KEY_TEXT_ANTIALIASING, RenderingHints.VALUE_TEXT_ANTIALIAS_ON);
    }

    public void draw(char c, float x, float baseline) {
        if (c == ' ') return;
        TextLayout tl = new TextLayout(String.valueOf(c), font, frc);
        Shape s = tl.getOutline(AffineTransform.getTranslateInstance(x, baseline));
        g.setColor(Color.BLACK);
        g.setStroke(new BasicStroke(stroke, BasicStroke.CAP_ROUND, BasicStroke.JOIN_ROUND));
        g.draw(s);
        g.setColor(Color.WHITE);
        g.fill(s);
    }

    public int[] finish() {
        g.dispose();
        int w = img.getWidth(), h = img.getHeight();
        int[] px = new int[w * h];
        img.getRGB(0, 0, w, h, px, 0, w);
        return px;
    }
}
