package com.hafij.inkwell.editor;

import com.hafij.inkwell.model.Doc;
import com.hafij.inkwell.model.Element;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;

/**
 * Snapshot undo. Elements are never changed in place once committed (edits replace them
 * with copies), so saving the old element lists is enough.
 */
public class Undo {
    private static final int LIMIT = 200;

    private static final class Entry {
        final Map<Doc.Layer, List<Element>> before = new IdentityHashMap<>();
        final Map<Doc.Layer, List<Element>> after = new IdentityHashMap<>();
        List<Doc.Page> pagesBefore, pagesAfter;
    }

    private final Doc doc;
    private final Deque<Entry> undo = new ArrayDeque<>();
    private final Deque<Entry> redo = new ArrayDeque<>();
    private Entry open;

    public Undo(Doc doc) {
        this.doc = doc;
    }

    public void begin() {
        if (open == null) open = new Entry();
    }

    /** Records the layer's current contents before the first change in this entry. */
    public void touch(Doc.Layer layer) {
        begin();
        if (!open.before.containsKey(layer)) open.before.put(layer, new ArrayList<>(layer.elements));
    }

    public void touchPages() {
        begin();
        if (open.pagesBefore == null) open.pagesBefore = new ArrayList<>(doc.pages);
    }

    /** Closes the open entry; returns false if nothing changed. */
    public boolean commit() {
        Entry e = open;
        open = null;
        if (e == null) return false;
        boolean changed = false;
        for (Map.Entry<Doc.Layer, List<Element>> b : e.before.entrySet()) {
            List<Element> now = new ArrayList<>(b.getKey().elements);
            e.after.put(b.getKey(), now);
            if (!now.equals(b.getValue())) changed = true;
        }
        if (e.pagesBefore != null) {
            e.pagesAfter = new ArrayList<>(doc.pages);
            if (!e.pagesAfter.equals(e.pagesBefore)) changed = true;
        }
        if (!changed) return false;
        undo.push(e);
        while (undo.size() > LIMIT) undo.removeLast();
        redo.clear();
        return true;
    }

    public boolean canUndo() {
        return !undo.isEmpty();
    }

    public boolean canRedo() {
        return !redo.isEmpty();
    }

    public boolean undo() {
        commit();
        Entry e = undo.poll();
        if (e == null) return false;
        apply(e.before, e.pagesBefore);
        redo.push(e);
        return true;
    }

    public boolean redo() {
        commit();
        Entry e = redo.poll();
        if (e == null) return false;
        apply(e.after, e.pagesAfter);
        undo.push(e);
        return true;
    }

    private void apply(Map<Doc.Layer, List<Element>> layers, List<Doc.Page> pages) {
        for (Map.Entry<Doc.Layer, List<Element>> m : layers.entrySet()) {
            m.getKey().elements = new ArrayList<>(m.getValue());
        }
        if (pages != null) {
            doc.pages.clear();
            doc.pages.addAll(pages);
        }
    }
}
