import loadRenderer from './index';

// Mermaid's own `run()` derives the id it stamps on the rendered `<svg>` from
// `Date.now()`, and its renderers then resolve that id with a document-wide
// lookup. Two diagrams whose renders begin in the same millisecond therefore
// draw into a single element: the first block shows both diagrams on top of
// each other, the second stays empty (#5023). Mint the id here instead —
// `render()` takes one — so concurrent renders can never share it.
let mermaidRenderCount = 0;

const RAPHAEL_DESC = /^Created with Rapha/;

// Give a fixed-size `<svg>` (one with `width`/`height` px attributes but no
// `viewBox`) a viewBox derived from those dimensions, so `max-width: 100%`
// scales it down to fit instead of clipping it. Returns false while the
// dimensions are still missing.
function sizeSvg(svg: Element): boolean {
    if (svg.getAttribute('viewBox'))
        return true;
    const width = Number.parseFloat(svg.getAttribute('width') ?? '');
    const height = Number.parseFloat(svg.getAttribute('height') ?? '');
    if (width > 0 && height > 0) {
        svg.setAttribute('viewBox', `0 0 ${width} ${height}`);
        return true;
    }
    return false;
}

// Draw an `<svg>` at its own width (mermaid's `max-width`, else its viewBox's) rather than
// shrunk to the column, so its text stays readable: a diagram wider than the column scrolls
// sideways instead (`.mu-diagram-preview` in blockSyntax.css).
function naturalWidth(svg: Element): void {
    if (!(svg instanceof SVGSVGElement))
        return;
    const viewBox = (svg.getAttribute('viewBox') ?? '').split(/[\s,]+/).map(Number);
    const fromStyle = Number.parseFloat(svg.style.maxWidth);
    const width = fromStyle > 0 ? fromStyle : viewBox[2] > 0 ? viewBox[2] : 0;
    if (!width)
        return;
    svg.style.width = `${width}px`;
    svg.style.maxWidth = 'none';
}

function accessibleName(media: Element, fallback: string): string {
    if (media.tagName.toLowerCase() !== 'svg')
        return fallback;

    const explicit = media.getAttribute('aria-label');
    if (explicit)
        return explicit;

    let title = '';
    let desc = '';
    for (const child of Array.from(media.children)) {
        const tag = child.tagName.toLowerCase();
        if (tag === 'title' && !title)
            title = (child.textContent ?? '').trim();
        else if (tag === 'desc' && !desc)
            desc = (child.textContent ?? '').trim();
    }
    // flowchart.js draws through Raphael, which stamps its own <desc> on every diagram.
    if (RAPHAEL_DESC.test(desc))
        desc = '';

    return [title, desc].filter(Boolean).join('. ') || fallback;
}

function applyFinalizations(target: HTMLElement, fallback: string): boolean {
    const media = target.querySelector('svg, img');
    if (!media)
        return false;
    if (media.tagName.toLowerCase() === 'svg') {
        if (!sizeSvg(media))
            return false;
        naturalWidth(media);
        // A diagram wider than the column opens on its middle, where a top-down chart's root is.
        if (target.scrollWidth > target.clientWidth)
            target.scrollLeft = (target.scrollWidth - target.clientWidth) / 2;
    }

    target.setAttribute('role', 'img');
    target.setAttribute('aria-label', accessibleName(media, fallback));

    return true;
}

// `drawSVG` (js-sequence-diagrams / flowchart.js) renders the `<svg>`
// asynchronously — it's drawn from a theme callback after its font loads — so
// neither the element nor its `width`/`height` attributes are there
// synchronously. Try once, then observe `target` until the diagram lands.
export function finalizeRenderedDiagram(target: HTMLElement, fallbackLabel: string): () => void {
    if (applyFinalizations(target, fallbackLabel))
        return () => {};

    const observer = new MutationObserver(() => {
        if (applyFinalizations(target, fallbackLabel))
            observer.disconnect();
    });
    observer.observe(target, {
        childList: true,
        subtree: true,
        attributes: true,
        attributeFilter: ['width', 'height'],
    });
    // Safety net so the observer can't leak if the diagram never renders.
    const timer = setTimeout(() => observer.disconnect(), 5000);

    // The caller owns this render: it must be able to give up on one that is
    // still waiting, rather than let it describe what replaced it.
    return () => {
        clearTimeout(timer);
        observer.disconnect();
    };
}

const FRONTMATTER = /^---\r?\n[\s\S]*?\r?\n---\r?\n/;

/**
 * Prefix `code` with a mermaid `%%{init}%%` directive so the config applies to
 * this one render. Mermaid's `initialize()` is process-global, and the live
 * preview re-initializes before every render, so mutating it around an export
 * would race whatever else is rendering.
 */
export function withMermaidInit(code: string, config: Record<string, unknown>): string {
    const directive = `%%{init: ${JSON.stringify(config)}}%%`;
    const frontmatter = FRONTMATTER.exec(code)?.[0] ?? '';
    const body = code.slice(frontmatter.length);

    return body.startsWith(directive)
        ? code
        : `${frontmatter}${directive}\n${body}`;
}

export interface IRenderOptions {
    type: string;
    code: string;
    target: HTMLElement;
    vegaTheme: string;
    mermaidTheme: string;
    plantumlServer: string;
    sequenceTheme: 'hand' | 'simple';
}

export interface IExportRenderOptions extends IRenderOptions {
    /**
     * Mermaid draws node labels into a `<foreignObject>` by default, and
     * Chromium refuses to paint one when the svg is loaded as an `<img>` —
     * rasterising such a diagram loses every label. Pass false to get a
     * `<text>`-only svg.
     */
    htmlLabels: boolean;
}

export async function renderDiagramForExport({
    htmlLabels,
    ...options
}: IExportRenderOptions): Promise<void> {
    const code = options.type === 'mermaid' && !htmlLabels
        ? withMermaidInit(options.code, { htmlLabels: false })
        : options.code;

    await renderDiagram({ ...options, code });
}

export async function renderDiagram({
    type,
    code,
    target,
    vegaTheme,
    mermaidTheme,
    plantumlServer,
    sequenceTheme,
}: IRenderOptions) {
    const render = await loadRenderer(type);
    const options = {};
    if (type === 'vega-lite') {
        Object.assign(options, {
            actions: false,
            tooltip: false,
            renderer: 'svg',
            theme: vegaTheme,
            ast: true,
        });
    }
    else if (type === 'sequence') {
        Object.assign(options, { theme: sequenceTheme });
    }

    if (type === 'plantuml') {
        const diagram = render.parse(code, plantumlServer);
        target.innerHTML = '';
        diagram.insertImgElement(target);
    }
    else if (type === 'vega-lite') {
        await render(target, JSON.parse(code), options);
    }
    else if (type === 'flowchart' || type === 'sequence') {
        const diagram = render.parse(code);
        target.innerHTML = '';
        diagram.drawSVG(target, options);
    }
    else if (type === 'mermaid') {
        render.initialize({
            startOnLoad: false,
            securityLevel: 'strict',
            theme: mermaidTheme,
        });
        await render.parse(code);
        // Mermaid sizes each box from its label's measured size, and that measurement includes
        // the CSS `zoom` of the target's ancestors (an editor whose page is zoomed), while the
        // drawn label doesn't: boxes came out too small or too large for their text. Render in an
        // unzoomed scratch box instead, then show the result in `target`.
        const scratch = document.createElement('div');
        scratch.style.cssText = 'position: absolute; top: 0; left: -100000px; visibility: hidden;';
        document.body.appendChild(scratch);
        let rendered: Awaited<ReturnType<typeof render.render>>;
        try {
            rendered = await render.render(
                `mu-mermaid-${++mermaidRenderCount}`,
                code,
                scratch,
            );
        }
        finally {
            scratch.remove();
        }
        const { svg, bindFunctions } = rendered;
        target.innerHTML = svg;
        bindFunctions?.(target);
    }
}
