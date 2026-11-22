package com.hafij.inkwell.model;

import android.graphics.Canvas;
import android.graphics.RectF;

/** Something drawn on a layer: a stroke, a text box or an image. Coordinates are PDF points. */
public abstract class Element {
    /** Bounding box in page coordinates, including the stroke width. */
    public abstract RectF bounds();

    public abstract void draw(Canvas canvas);

    public abstract void translate(float dx, float dy);

    /** Scales about (px, py). */
    public abstract void scale(float sx, float sy, float px, float py);

    public abstract Element copy();

    /** True if the point is within {@code radius} of the visible element. */
    public abstract boolean hit(float x, float y, float radius);

    /** Colour as ARGB, used by recolouring. */
    public abstract int getColor();

    public abstract void setColor(int argb);
}
