// Word-style ribbon adapted for touch: same tabs and groups as the desktop
// ribbon (packages/desktop/.../formatToolbar), larger hit targets, and a body
// that scrolls sideways. Buttons cancel `mousedown` so tapping them does not
// move focus out of the editor (which would drop the selection and close the
// soft keyboard).

import { ICONS } from './icons'
import * as cmd from './commands'
import { newDocument, openDocument, saveDocument, saveDocumentAs } from './document'
import { openDrawer } from './drawer'
import { exportDocx, exportHtml, exportPdf, shareDocument } from './export'
import { settings, updateSettings, type PageLayoutId, type ThemeChoice } from './settings'
import { h } from './ui'

type TabId = 'file' | 'home' | 'insert' | 'draw' | 'layout' | 'view'

interface ToolItem {
  id: string
  label: string
  glyph: string
  size?: 'large' | 'small'
  run: () => void
  isActive?: () => boolean
}

interface ToolGroup {
  id: string
  label: string
  layout?: 'grid' | 'row'
  items?: ToolItem[]
  custom?: () => HTMLElement[]
}

const TABS: Array<{ id: TabId, label: string }> = [
  { id: 'file', label: 'File' },
  { id: 'home', label: 'Home' },
  { id: 'insert', label: 'Insert' },
  { id: 'draw', label: 'Draw' },
  { id: 'layout', label: 'Layout' },
  { id: 'view', label: 'View' }
]

const STYLE_TILES = [
  { id: 'normal', label: 'Normal', type: 'paragraph' },
  { id: 'heading1', label: 'Heading 1', type: 'heading 1' },
  { id: 'heading2', label: 'Heading 2', type: 'heading 2' },
  { id: 'heading3', label: 'Heading 3', type: 'heading 3' },
  { id: 'heading4', label: 'Heading 4', type: 'heading 4' },
  { id: 'quote', label: 'Quote', type: 'blockquote' }
]

const MARGINS = [
  { id: 'narrow', label: 'Narrow', mm: 12.7 },
  { id: 'moderate', label: 'Moderate', mm: 19.05 },
  { id: 'normal', label: 'Normal', mm: 25.4 },
  { id: 'wide', label: 'Wide', mm: 50.8 }
]

const large = (id: string, label: string, glyph: string, run: () => void, isActive?: () => boolean): ToolItem =>
  ({ id, label, glyph, run, isActive, size: 'large' })
const small = (id: string, label: string, glyph: string, run: () => void, isActive?: () => boolean): ToolItem =>
  ({ id, label, glyph, run, isActive, size: 'small' })

const setLayout = (pageLayout: PageLayoutId) => (): void => {
  if (cmd.isSourceMode()) cmd.toggleSource()
  updateSettings({ pageLayout })
}

const setTheme = (theme: ThemeChoice) => (): void => updateSettings({ theme })

const GROUPS: Record<TabId, ToolGroup[]> = {
  file: [
    {
      id: 'document',
      label: 'Document',
      layout: 'row',
      items: [
        large('new', 'New', ICONS.newFile(), () => newDocument()),
        large('open', 'Open', ICONS.open(), () => openDocument()),
        large('browse', 'Recent & folders', ICONS.recent(), openDrawer),
        large('save', 'Save', ICONS.save.replace(/width="20" height="20"/, 'width="28" height="28"'), () => saveDocument()),
        large('saveAs', 'Save as', ICONS.saveAs(), () => saveDocumentAs()),
        large('share', 'Share', ICONS.share(), shareDocument)
      ]
    },
    {
      id: 'export',
      label: 'Export',
      layout: 'row',
      items: [
        large('exportDocx', 'Word', ICONS.word(), exportDocx),
        large('exportPdf', 'PDF', ICONS.pdf(), exportPdf),
        large('exportHtml', 'HTML', ICONS.htmlExport(), exportHtml)
      ]
    },
    {
      id: 'options',
      label: 'Options',
      layout: 'row',
      items: [
        large('autoSave', 'AutoSave', ICONS.check(28), () => updateSettings({ autoSave: !settings.autoSave }), () => settings.autoSave)
      ]
    }
  ],
  home: [
    {
      id: 'undo',
      label: 'Undo',
      layout: 'grid',
      items: [
        small('undo', 'Undo', ICONS.undo, cmd.undo),
        small('redo', 'Redo', ICONS.redo, cmd.redo)
      ]
    },
    {
      id: 'font',
      label: 'Font',
      layout: 'grid',
      items: [
        small('bold', 'Bold', '<b>B</b>', cmd.format('strong')),
        small('italic', 'Italic', '<i>I</i>', cmd.format('em')),
        small('underline', 'Underline', '<u>U</u>', cmd.format('u')),
        small('strikethrough', 'Strikethrough', '<s>ab</s>', cmd.format('del')),
        small('subscript', 'Subscript', 'x<sub>2</sub>', cmd.format('sub')),
        small('superscript', 'Superscript', 'x<sup>2</sup>', cmd.format('sup')),
        small('highlight', 'Highlight', '<span class="hl">ab</span>', cmd.format('mark')),
        small('inlineCode', 'Inline code', ICONS.code, cmd.format('inline_code')),
        small('inlineMath', 'Inline math', '∑', cmd.format('inline_math')),
        small('clearFormat', 'Clear formatting', ICONS.clear, cmd.format('clear'))
      ]
    },
    {
      id: 'paragraph',
      label: 'Paragraph',
      layout: 'grid',
      items: [
        small('bulletList', 'Bullets', ICONS.bullets, cmd.paragraph('ul-bullet')),
        small('orderedList', 'Numbering', ICONS.numbers, cmd.paragraph('ol-order')),
        small('taskList', 'Task list', ICONS.tasks, cmd.paragraph('ul-task')),
        small('decreaseHeading', 'Smaller heading', 'A<sup>−</sup>', cmd.paragraph('degrade heading')),
        small('increaseHeading', 'Larger heading', 'A<sup>+</sup>', cmd.paragraph('upgrade heading')),
        small('quote', 'Quote', ICONS.quote(), cmd.paragraph('blockquote')),
        small('codeBlock', 'Code block', ICONS.codeBlock(), cmd.paragraph('pre')),
        small('horizontalLine', 'Horizontal line', ICONS.hr(), cmd.paragraph('hr'))
      ]
    },
    {
      id: 'styles',
      label: 'Styles',
      custom: () => STYLE_TILES.map((style) =>
        h('button', {
          type: 'button',
          class: `style-tile ${style.id}`,
          title: style.label,
          onClick: cmd.paragraph(style.type)
        }, h('span', { class: 'style-preview' }, 'AaBbCc'), h('span', { class: 'style-name' }, style.label))
      )
    },
    {
      id: 'editing',
      label: 'Editing',
      layout: 'grid',
      items: [
        small('find', 'Find', ICONS.find(), () => cmd.openFind(false)),
        small('replace', 'Replace', ICONS.replace(), () => cmd.openFind(true)),
        small('selectAll', 'Select all', ICONS.selectAll(), cmd.selectAll),
        small('fontBigger', 'Larger text', 'A<sup>▲</sup>', () => cmd.setFontSize(1))
      ]
    }
  ],
  insert: [
    {
      id: 'tables',
      label: 'Tables',
      layout: 'row',
      items: [large('table', 'Table', ICONS.table(28), cmd.insertTable)]
    },
    {
      id: 'illustrations',
      label: 'Illustrations',
      layout: 'row',
      items: [
        large('image', 'Pictures', ICONS.image(28), cmd.insertPicture),
        large('sketch', 'Sketch', ICONS.sketch(), cmd.insertSketch),
        large('link', 'Link', ICONS.link(28), cmd.insertLink)
      ]
    },
    {
      id: 'blocks',
      label: 'Blocks',
      layout: 'row',
      items: [
        large('codeBlock', 'Code', ICONS.codeBlock(28), cmd.paragraph('pre')),
        large('mathBlock', 'Equation', ICONS.math(28), cmd.paragraph('mathblock')),
        large('quote', 'Quote', ICONS.quote(28), cmd.paragraph('blockquote')),
        large('horizontalLine', 'Line', ICONS.hr(28), cmd.paragraph('hr')),
        large('htmlBlock', 'HTML', ICONS.html(28), cmd.paragraph('html')),
        large('frontMatter', 'Front matter', ICONS.frontMatter(28), cmd.paragraph('front-matter'))
      ]
    }
  ],
  draw: [
    {
      id: 'ink',
      label: 'Ink',
      layout: 'row',
      items: [
        large('sketch', 'Sketch', ICONS.sketch(), cmd.insertSketch),
        large('handwriting', 'Pen to text', ICONS.handwriting(), cmd.toggleHandwriting, () => settings.penHandwriting),
        large('fingerDrawing', 'Draw with finger', ICONS.hand(), () => updateSettings({ fingerDrawing: !settings.fingerDrawing }), () => settings.fingerDrawing)
      ]
    },
    {
      id: 'sketchOptions',
      label: 'Sketch image',
      layout: 'row',
      items: [
        large('png', 'PNG', ICONS.image(28), () => updateSettings({ sketchFormat: 'png' }), () => settings.sketchFormat === 'png'),
        large('svg', 'SVG', ICONS.pen(), () => updateSettings({ sketchFormat: 'svg' }), () => settings.sketchFormat === 'svg'),
        large('paper', 'White paper', ICONS.page(), () => updateSettings({ sketchPaper: !settings.sketchPaper }), () => settings.sketchPaper)
      ]
    },
    {
      id: 'help',
      label: 'Help',
      layout: 'row',
      items: [large('penHelp', 'Pen tips', ICONS.find(28), cmd.showHandwritingHelp)]
    }
  ],
  layout: [
    {
      id: 'pageSize',
      label: 'Size',
      layout: 'row',
      items: (['a4', 'letter', 'legal'] as const).map((size) =>
        large(`size_${size}`, size === 'a4' ? 'A4' : size[0]!.toUpperCase() + size.slice(1), ICONS.page(), setLayout(size), () => settings.pageLayout === size)
      )
    },
    {
      id: 'margins',
      label: 'Margins',
      layout: 'row',
      items: MARGINS.map((margin) =>
        large(`margin_${margin.id}`, margin.label, ICONS.margins(), () => {
          updateSettings({ pageMargin: margin.mm, pageLayout: settings.pageLayout === 'none' ? 'a4' : settings.pageLayout })
        }, () => settings.pageLayout !== 'none' && Math.abs(settings.pageMargin - margin.mm) < 0.01)
      )
    }
  ],
  view: [
    {
      id: 'views',
      label: 'Views',
      layout: 'row',
      items: [
        large('printLayout', 'Print layout', ICONS.page(), () => setLayout(settings.pageLayout === 'none' ? 'a4' : settings.pageLayout)(),
          () => !cmd.isSourceMode() && settings.pageLayout !== 'none'),
        large('mobileLayout', 'Mobile view', ICONS.web(), setLayout('none'), () => !cmd.isSourceMode() && settings.pageLayout === 'none'),
        large('source', 'Source', ICONS.source(), cmd.toggleSource, cmd.isSourceMode),
        large('focus', 'Focus', ICONS.focus(), cmd.toggleFocusMode, () => settings.focusMode)
      ]
    },
    {
      id: 'zoom',
      label: 'Zoom',
      layout: 'row',
      items: [
        large('zoomFit', 'Page width', ICONS.zoomFit(), () => updateSettings({ zoom: 0 }), () => settings.zoom === 0),
        large('zoom100', '100%', ICONS.zoom100(), () => updateSettings({ zoom: 1 }), () => settings.zoom === 1)
      ]
    },
    {
      id: 'theme',
      label: 'Theme',
      layout: 'row',
      items: [
        large('system', 'System', ICONS.system(), setTheme('system'), () => settings.theme === 'system'),
        large('light', 'Word Light', ICONS.sun(), setTheme('word'), () => settings.theme === 'word'),
        large('dark', 'Word Dark', ICONS.moon(), setTheme('word-dark'), () => settings.theme === 'word-dark')
      ]
    }
  ]
}

const preventFocusSteal = (el: HTMLElement): void => {
  el.addEventListener('mousedown', (event) => event.preventDefault())
}

const renderTool = (item: ToolItem): HTMLElement => {
  const active = item.isActive?.()
  const button = h('button', {
    type: 'button',
    class: `tool ${item.size ?? 'small'}${active ? ' active' : ''}`,
    title: item.label,
    'aria-label': item.label,
    'aria-pressed': item.isActive ? String(!!active) : undefined,
    onClick: () => {
      item.run()
      if (item.isActive) requestAnimationFrame(renderBody)
    }
  }, h('span', { class: 'glyph', html: item.glyph }))
  if (item.size === 'large') button.append(h('span', { class: 'label' }, item.label))
  return button
}

let activeTab: TabId = TABS.some((tab) => tab.id === settings.ribbonTab) ? (settings.ribbonTab as TabId) : 'home'

const renderBody = (): void => {
  const body = document.getElementById('ribbon-body')!
  const scroll = body.scrollLeft
  body.replaceChildren()
  for (const group of GROUPS[activeTab]) {
    const items = group.custom ? group.custom() : (group.items ?? []).map(renderTool)
    body.append(h('div', { class: 'ribbon-group', role: 'group', 'aria-label': group.label },
      h('div', { class: `group-items ${group.layout ?? 'row'}` }, ...items),
      h('div', { class: 'group-caption' }, group.label)
    ))
  }
  body.scrollLeft = scroll
  body.hidden = settings.ribbonCollapsed
}

const renderTabs = (): void => {
  const tabs = document.getElementById('ribbon-tabs')!
  tabs.replaceChildren(
    ...TABS.map((tab) => h('button', {
      type: 'button',
      role: 'tab',
      class: `ribbon-tab${tab.id === activeTab && !settings.ribbonCollapsed ? ' active' : ''}`,
      'aria-selected': String(tab.id === activeTab),
      onClick: () => {
        if (tab.id === activeTab) updateSettings({ ribbonCollapsed: !settings.ribbonCollapsed })
        else {
          activeTab = tab.id
          updateSettings({ ribbonTab: tab.id, ribbonCollapsed: false })
          document.getElementById('ribbon-body')!.scrollLeft = 0
        }
        renderTabs()
        renderBody()
      }
    }, tab.label)),
    h('button', {
      type: 'button',
      class: 'ribbon-collapse',
      'aria-label': settings.ribbonCollapsed ? 'Expand ribbon' : 'Collapse ribbon',
      html: settings.ribbonCollapsed ? ICONS.chevronDown : ICONS.chevronUp,
      onClick: () => {
        updateSettings({ ribbonCollapsed: !settings.ribbonCollapsed })
        renderTabs()
        renderBody()
      }
    })
  )
}

export const refreshRibbon = (): void => {
  renderTabs()
  renderBody()
}

export const initRibbon = (): void => {
  const ribbon = document.getElementById('ribbon')!
  preventFocusSteal(ribbon)
  refreshRibbon()
  document.addEventListener('markd:view', renderBody)
}
