package com.hafij.inkwell;

import android.content.Context;

import com.tom_roush.pdfbox.android.PDFBoxResourceLoader;

/** One-time setup for the Inkwell screens inside the host app. */
public final class Inkwell {
    private static boolean ready;

    private Inkwell() {
    }

    public static synchronized void init(Context c) {
        if (ready) return;
        PDFBoxResourceLoader.init(c.getApplicationContext());
        ready = true;
    }
}
