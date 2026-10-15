<template>
  <div
    class="ribbon"
    :class="{ collapsed }"
  >
    <div
      class="ribbon-tabs"
      role="tablist"
      :aria-label="t('formatToolbar.label')"
    >
      <button
        v-for="tab in tabs"
        :key="tab.id"
        type="button"
        role="tab"
        class="ribbon-tab"
        :class="{ active: activeTab === tab.id && !collapsed }"
        :aria-selected="activeTab === tab.id"
        @mousedown.prevent
        @click="selectTab(tab.id)"
        @dblclick="collapsed = !collapsed"
      >
        {{ t(`formatToolbar.tabs.${tab.id}`) }}
      </button>
      <button
        type="button"
        class="ribbon-collapse"
        :title="t(collapsed ? 'formatToolbar.expand' : 'formatToolbar.collapse')"
        :aria-label="t(collapsed ? 'formatToolbar.expand' : 'formatToolbar.collapse')"
        @mousedown.prevent
        @click="collapsed = !collapsed"
      >
        <span v-html="collapsed ? ICONS.chevronDown : ICONS.chevronUp" />
      </button>
    </div>

    <div
      v-show="!collapsed"
      class="ribbon-body"
      role="toolbar"
      @wheel="scrollSideways"
    >
      <div
        v-for="group in currentGroups"
        :key="group.id"
        class="ribbon-group"
      >
        <div
          class="group-items"
          :class="group.layout"
        >
          <template v-if="group.id === 'themes'">
            <div class="theme-gallery">
              <button
                v-for="item in THEME_TILES"
                :key="item.id"
                type="button"
                class="theme-tile"
                :class="{ active: !followSystemTheme && theme === item.id }"
                :title="item.label"
                @mousedown.prevent
                @click="selectTheme(item.id)"
              >
                <span
                  class="theme-swatch"
                  :style="{ background: item.bg, color: item.fg }"
                >Aa</span>
                <span class="theme-name">{{ item.label }}</span>
              </button>
            </div>
          </template>
          <template v-else-if="group.id === 'styles'">
            <button
              v-for="style in STYLE_TILES"
              :key="style.id"
              type="button"
              class="style-tile"
              :class="style.id"
              :title="t(`formatToolbar.${style.id}`)"
              @mousedown.prevent
              @click="bus.emit('paragraph', style.type)"
            >
              <span class="style-preview">AaBbCc</span>
              <span class="style-name">{{ t(`formatToolbar.${style.id}`) }}</span>
            </button>
          </template>
          <template v-else>
            <button
              v-for="item in group.items"
              :key="item.id"
              type="button"
              class="tool"
              :class="[item.size ?? 'small', { active: item.isActive?.() }]"
              :title="t(`formatToolbar.${item.id}`)"
              :aria-label="t(`formatToolbar.${item.id}`)"
              :aria-pressed="item.isActive ? !!item.isActive() : undefined"
              @mousedown.prevent
              @click="item.run()"
            >
              <span
                class="glyph"
                v-html="item.glyph"
              />
              <span
                v-if="item.size === 'large' || item.showLabel"
                class="label"
              >{{ t(`formatToolbar.${item.id}`) }}</span>
            </button>
          </template>
        </div>
        <div class="group-caption">
          {{ t(`formatToolbar.groups.${group.id}`) }}
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { storeToRefs } from 'pinia'
import bus from '@/bus'
import { usePreferencesStore } from '@/store/preferences'
import { useLayoutStore } from '@/store/layout'
import commands, { type CommandDescriptor } from '@/commands'
import { quickExport } from '@/util/quickExport'
import { themes as THEME_LIST } from '@/prefComponents/theme/config'
import { getThemeBackgroundColor, isDarkThemeId } from 'common/theme'

const { t } = useI18n()
const preferencesStore = usePreferencesStore()
const { pageLayout, pageMargin, typewriter, focus, sourceCode, theme, followSystemTheme, autoSave, zoom } =
  storeToRefs(preferencesStore)
const { showSideBar, showTabBar } = storeToRefs(useLayoutStore())

type TabId = 'file' | 'home' | 'insert' | 'design' | 'layout' | 'view' | 'help'

interface ToolItem {
  id: string
  glyph: string
  size?: 'large' | 'small'
  showLabel?: boolean
  run: () => void
  isActive?: () => boolean
}

interface ToolGroup {
  id: string
  layout?: 'grid' | 'row' | 'column'
  items: ToolItem[]
}

// Static icon markup (no user input), rendered with v-html.
const svg = (d: string, size = 16): string =>
  `<svg viewBox="0 0 24 24" width="${size}" height="${size}" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">${d}</svg>`

const ICONS = {
  chevronUp: svg('<path d="m18 15-6-6-6 6"/>', 14),
  chevronDown: svg('<path d="m6 9 6 6 6-6"/>', 14),
  undo: svg('<path d="M9 14 4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/>'),
  redo: svg('<path d="m15 14 5-5-5-5"/><path d="M20 9H9.5a5.5 5.5 0 0 0 0 11H13"/>'),
  code: svg('<path d="m16 18 6-6-6-6"/><path d="m8 6-6 6 6 6"/>'),
  clear: svg('<path d="M4 7V4h16v3"/><path d="M5 20h6"/><path d="M13 4 8 20"/><path d="m15 15 5 5"/><path d="m20 15-5 5"/>'),
  link: (s = 16) => svg('<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>', s),
  image: (s = 16) => svg('<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.1-3.1a2 2 0 0 0-2.8 0L6 21"/>', s),
  table: (s = 16) => svg('<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18M3 15h18M9 3v18M15 3v18"/>', s),
  bullets: svg('<path d="M9 6h12M9 12h12M9 18h12"/><circle cx="4" cy="6" r="1"/><circle cx="4" cy="12" r="1"/><circle cx="4" cy="18" r="1"/>'),
  numbers: svg('<path d="M10 6h11M10 12h11M10 18h11"/><path d="M4 6h1v4"/><path d="M4 10h2"/><path d="M6 18H4c0-1 2-2 2-3s-1-1.5-2-1"/>'),
  tasks: svg('<rect x="3" y="5" width="6" height="6" rx="1"/><path d="m3 17 2 2 4-4"/><path d="M13 6h8M13 12h8M13 18h8"/>'),
  quote: (s = 16) => svg('<path d="M3 21c3 0 7-1 7-8V5H3v8h4"/><path d="M14 21c3 0 7-1 7-8V5h-7v8h4"/>', s),
  codeBlock: (s = 16) => svg('<rect x="3" y="4" width="18" height="16" rx="2"/><path d="m10 10-2 2 2 2"/><path d="m14 10 2 2-2 2"/>', s),
  hr: (s = 16) => svg('<path d="M3 12h18"/><path d="M3 6h18" opacity=".35"/><path d="M3 18h18" opacity=".35"/>', s),
  math: (s = 16) => svg('<path d="M18 4H6l6 8-6 8h12"/>', s),
  html: (s = 16) => svg('<path d="m7 8-4 4 4 4"/><path d="m17 8 4 4-4 4"/><path d="m14 4-4 16"/>', s),
  frontMatter: (s = 16) => svg('<path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><path d="M14 3v6h6"/><path d="M8 13h8M8 17h5"/>', s),
  find: (s = 16) => svg('<circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/>', s),
  replace: (s = 16) => svg('<path d="M4 7h11l-3-3"/><path d="M20 17H9l3 3"/>', s),
  page: (s = 24) => svg('<path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><path d="M14 3v6h6"/>', s),
  web: (s = 24) => svg('<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9h18"/>', s),
  margins: (s = 24) => svg('<rect x="4" y="3" width="16" height="18" rx="1"/><rect x="8" y="7" width="8" height="10" stroke-dasharray="2 2"/>', s),
  source: (s = 24) => svg('<path d="m8 9-3 3 3 3"/><path d="m16 9 3 3-3 3"/><rect x="2" y="3" width="20" height="18" rx="2"/>', s),
  focus: (s = 24) => svg('<circle cx="12" cy="12" r="3"/><path d="M3 8V5a2 2 0 0 1 2-2h3M16 3h3a2 2 0 0 1 2 2v3M21 16v3a2 2 0 0 1-2 2h-3M8 21H5a2 2 0 0 1-2-2v-3"/>', s),
  typewriter: (s = 24) => svg('<rect x="2" y="9" width="20" height="10" rx="2"/><path d="M6 9V4h12v5"/><path d="M6 14h.01M10 14h.01M14 14h.01M18 14h.01"/>', s),
  sidebar: (s = 24) => svg('<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18"/>', s),
  tabsIcon: (s = 24) => svg('<path d="M3 8h6l2-3h10v14H3z"/>', s),
  pdf: (s = 24) => svg('<path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><path d="M14 3v6h6"/><text x="7" y="18" font-size="6" fill="currentColor" stroke="none" font-family="sans-serif" font-weight="700">PDF</text>', s),
  word: (s = 24) => svg('<path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><path d="M14 3v6h6"/><path d="m7 12 1.5 6 2-4.5 2 4.5L14 12"/>', s),
  newDoc: (s = 24) => svg('<path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><path d="M14 3v6h6"/><path d="M12 12v6M9 15h6"/>', s),
  newWindow: (s = 24) => svg('<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9h18M12 12v5M9.5 14.5h5"/>', s),
  open: (s = 24) => svg('<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v1H7l-3 9H3z"/><path d="M7 10h14l-3 9H4"/>', s),
  folder: (s = 24) => svg('<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>', s),
  importDoc: (s = 24) => svg('<path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><path d="M12 10v7M9 14l3 3 3-3"/>', s),
  save: (s = 24) => svg('<path d="M5 3h11l5 5v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"/><path d="M7 3v5h8V3M7 21v-7h10v7"/>', s),
  saveAs: (s = 24) => svg('<path d="M5 3h11l5 5v4"/><path d="M3 5v14a2 2 0 0 0 2 2h6"/><path d="M7 3v5h8V3"/><path d="m14 19 6-6 2 2-6 6h-2z"/>', s),
  rename: (s = 24) => svg('<path d="M4 20h4L19 9l-4-4L4 16z"/><path d="m13 7 4 4"/>', s),
  move: (s = 24) => svg('<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><path d="M9 13h7M13 10l3 3-3 3"/>', s),
  autoSave: (s = 24) => svg('<path d="M21 12a9 9 0 1 1-3-6.7L21 8"/><path d="M21 3v5h-5"/><path d="M12 7v5l3 2"/>', s),
  print: (s = 24) => svg('<path d="M6 9V3h12v6"/><rect x="3" y="9" width="18" height="8" rx="2"/><path d="M6 14h12v7H6z"/>', s),
  closeTab: (s = 24) => svg('<rect x="3" y="5" width="18" height="14" rx="2"/><path d="m9 9 6 6M15 9l-6 6"/>', s),
  closeWindow: (s = 24) => svg('<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 8h18M10 12l4 4M14 12l-4 4"/>', s),
  settings: (s = 16) => svg('<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 0 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 0 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 0 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 0 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>', s),
  quit: (s = 24) => svg('<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><path d="m16 17 5-5-5-5M21 12H9"/>', s),
  paste: (s = 16) => svg('<rect x="8" y="2" width="8" height="4" rx="1"/><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>', s),
  pastePlain: (s = 16) => svg('<rect x="8" y="2" width="8" height="4" rx="1"/><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/><path d="M9 12h6M12 12v6"/>', s),
  cut: (s = 16) => svg('<circle cx="6" cy="18" r="3"/><circle cx="18" cy="18" r="3"/><path d="M8.1 16 20 4M15.9 16 4 4"/>', s),
  copy: (s = 16) => svg('<rect x="8" y="8" width="13" height="13" rx="2"/><path d="M4 16V5a2 2 0 0 1 2-2h11"/>', s),
  copyRich: (s = 16) => svg('<rect x="8" y="8" width="13" height="13" rx="2"/><path d="M4 16V5a2 2 0 0 1 2-2h11"/><path d="M12 12h5M12 16h3"/>', s),
  copyHtml: (s = 16) => svg('<rect x="8" y="8" width="13" height="13" rx="2"/><path d="M4 16V5a2 2 0 0 1 2-2h11"/><path d="m13 12-2 2.5 2 2.5M16 12l2 2.5-2 2.5"/>', s),
  folderSearch: (s = 16) => svg('<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v3"/><path d="M3 7v10a2 2 0 0 0 2 2h6"/><circle cx="17" cy="17" r="3"/><path d="m22 22-2.5-2.5"/>', s),
  selectAll: (s = 16) => svg('<path d="M5 3a2 2 0 0 0-2 2M19 3a2 2 0 0 1 2 2M21 19a2 2 0 0 1-2 2M5 21a2 2 0 0 1-2-2M9 3h1M14 3h1M9 21h1M14 21h1M3 9v1M3 14v1M21 9v1M21 14v1"/><rect x="7" y="7" width="10" height="10" rx="1"/>', s),
  duplicate: (s = 16) => svg('<rect x="8" y="8" width="13" height="13" rx="2"/><path d="M4 16V5a2 2 0 0 1 2-2h11M14.5 11.5v6M11.5 14.5h6"/>', s),
  newPara: (s = 16) => svg('<path d="M4 6h16M4 10h10"/><path d="M17 14v6M14 17h6"/>', s),
  deletePara: (s = 16) => svg('<path d="M4 6h16M4 10h10"/><path d="M14 17h6"/>', s),
  screenshot: (s = 16) => svg('<path d="M3 7V5a2 2 0 0 1 2-2h2M17 3h2a2 2 0 0 1 2 2v2M21 17v2a2 2 0 0 1-2 2h-2M7 21H5a2 2 0 0 1-2-2v-2"/><circle cx="12" cy="12" r="3"/>', s),
  contrast: (s = 16) => svg('<circle cx="12" cy="12" r="9"/><path d="M12 3v18a9 9 0 0 0 0-18z" fill="currentColor"/>', s),
  zoomIn: (s = 24) => svg('<circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3M11 8v6M8 11h6"/>', s),
  zoomOut: (s = 24) => svg('<circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3M8 11h6"/>', s),
  zoomReset: (s = 24) => svg('<circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/><text x="7" y="14" font-size="7" fill="currentColor" stroke="none" font-family="sans-serif" font-weight="700">1:1</text>', s),
  fullScreen: (s = 24) => svg('<path d="M8 3H5a2 2 0 0 0-2 2v3M21 8V5a2 2 0 0 0-2-2h-3M3 16v3a2 2 0 0 0 2 2h3M16 21h3a2 2 0 0 0 2-2v-3"/>', s),
  pin: (s = 24) => svg('<path d="M12 17v5M9 3h6l-1 6 4 4H6l4-4z"/>', s),
  minimize: (s = 24) => svg('<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M8 15h8"/>', s),
  ltr: (s = 24) => svg('<path d="M9 4v12M13 4v12M9 4h6M6 4a3 3 0 0 0 0 6h3"/><path d="M5 20h14M16 17l3 3-3 3"/>', s),
  rtl: (s = 24) => svg('<path d="M11 4v12M15 4v12M11 4h6M8 4a3 3 0 0 0 0 6h3"/><path d="M19 20H5M8 17l-3 3 3 3"/>', s),
  book: (s = 24) => svg('<path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20V3H6.5A2.5 2.5 0 0 0 4 5.5z"/><path d="M4 19.5A2.5 2.5 0 0 0 6.5 22H20v-5"/>', s),
  markdown: (s = 24) => svg('<rect x="2" y="5" width="20" height="14" rx="2"/><path d="M6 15V9l3 3 3-3v6M17 9v6M15 13l2 2 2-2"/>', s),
  palette: (s = 24) => svg('<rect x="3" y="4" width="18" height="16" rx="2"/><path d="m7 9 3 3-3 3M12 15h5"/>', s),
  update: (s = 24) => svg('<path d="M21 12a9 9 0 1 1-3-6.7L21 8"/><path d="M21 3v5h-5"/>', s),
  info: (s = 24) => svg('<circle cx="12" cy="12" r="9"/><path d="M12 16v-4M12 8h.01"/>', s),
  htmlExport: (s = 24) => svg('<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/>', s)
}

const tabs: Array<{ id: TabId }> = [
  { id: 'file' },
  { id: 'home' },
  { id: 'insert' },
  { id: 'design' },
  { id: 'layout' },
  { id: 'view' },
  { id: 'help' }
]

const STORAGE_KEY = 'markd.ribbon'
const loadState = (): { tab: TabId; collapsed: boolean } => {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) || '{}')
    return {
      tab: tabs.some((tab) => tab.id === saved.tab) ? saved.tab : 'home',
      collapsed: !!saved.collapsed
    }
  } catch {
    return { tab: 'home', collapsed: false }
  }
}
const initial = loadState()
const activeTab = ref<TabId>(initial.tab)
const collapsed = ref(initial.collapsed)
watch([activeTab, collapsed], () => {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ tab: activeTab.value, collapsed: collapsed.value }))
  } catch {
    // Ignore storage failures (private mode, quota).
  }
})

const selectTab = (id: TabId): void => {
  if (collapsed.value && activeTab.value === id) {
    collapsed.value = false
  }
  activeTab.value = id
}

const format = (type: string) => () => bus.emit('format', type)
const paragraph = (type: string) => () => bus.emit('paragraph', type)
const setPref = (type: string, value: unknown): void => {
  preferencesStore.SET_SINGLE_PREFERENCE({ type: type as never, value })
}
const toggleView = (entry: 'typewriter' | 'focus' | 'sourceCode') => () =>
  bus.emit('view:toggle-view-entry', entry)
const toggleLayout = (entry: 'showSideBar' | 'showTabBar') => () =>
  bus.emit('view:toggle-layout-entry', entry)
const exportAs = (type: 'pdf' | 'docx' | 'styledHtml' | 'print') => () => quickExport(type)

const findCommand = (list: CommandDescriptor[], id: string): CommandDescriptor | undefined => {
  for (const item of list) {
    if (item.id === id) return item
    const sub = item.subcommands ? findCommand(item.subcommands as CommandDescriptor[], id) : undefined
    if (sub) return sub
  }
  return undefined
}
const command = (id: string) => () => {
  void findCommand(commands, id)?.execute?.()
}
const editRole = (role: 'cut' | 'copy' | 'paste' | 'selectAll') => () =>
  window.electron.ipcRenderer.send('mt::cmd-edit-role', role)

const ZOOM_STEPS = [0.625, 0.75, 0.875, 1, 1.125, 1.25, 1.375, 1.5, 1.625, 1.75, 1.875, 2]
const stepZoom = (direction: 1 | -1) => () => {
  const current = zoom.value || 1
  const next =
    direction > 0
      ? ZOOM_STEPS.find((step) => step > current + 0.001)
      : [...ZOOM_STEPS].reverse().find((step) => step < current - 0.001)
  if (next) bus.emit('mt::window-zoom', next)
}

const themeLabel = (id: string): string => {
  if (id === 'light') return 'Cadmium Light'
  if (id === 'dark') return 'Cadmium Dark'
  return id
    .split('-')
    .map((part) => (/^\d/.test(part) ? `'${part}` : part.charAt(0).toUpperCase() + part.slice(1)))
    .join(' ')
    .replace(/^Github/, 'GitHub')
}
const THEME_TILES = THEME_LIST.map(({ name }) => ({
  id: name,
  label: themeLabel(name),
  bg: getThemeBackgroundColor(name),
  fg: isDarkThemeId(name) ? '#e6e6e6' : '#262626'
}))
const selectTheme = (id: string): void => {
  window.electron.ipcRenderer.send('mt::set-user-preference', { followSystemTheme: false, theme: id })
}

const STYLE_TILES = [
  { id: 'normal', type: 'paragraph' },
  { id: 'heading1', type: 'heading 1' },
  { id: 'heading2', type: 'heading 2' },
  { id: 'heading3', type: 'heading 3' },
  { id: 'heading4', type: 'heading 4' },
  { id: 'quote', type: 'blockquote' }
]

const MARGINS = [
  { id: 'marginNarrow', mm: 12.7 },
  { id: 'marginModerate', mm: 19.05 },
  { id: 'marginNormal', mm: 25.4 },
  { id: 'marginWide', mm: 50.8 }
]

const GROUPS: Record<TabId, ToolGroup[]> = {
  file: [
    {
      id: 'new',
      layout: 'row',
      items: [
        { id: 'newTab', size: 'large', glyph: ICONS.newDoc(28), run: command('file.new-tab') },
        { id: 'newWindow', size: 'large', glyph: ICONS.newWindow(28), run: command('file.new-window') }
      ]
    },
    {
      id: 'open',
      layout: 'row',
      items: [
        { id: 'openFile', size: 'large', glyph: ICONS.open(28), run: command('file.open-file') },
        { id: 'openFolder', size: 'large', glyph: ICONS.folder(28), run: command('file.open-folder') },
        { id: 'importFile', size: 'large', glyph: ICONS.importDoc(28), run: command('file.import-file') }
      ]
    },
    {
      id: 'save',
      layout: 'row',
      items: [
        { id: 'saveFile', size: 'large', glyph: ICONS.save(28), run: command('file.save') },
        { id: 'saveAs', size: 'large', glyph: ICONS.saveAs(28), run: command('file.save-as') },
        { id: 'renameFile', size: 'large', glyph: ICONS.rename(28), run: command('file.rename-file') },
        { id: 'moveFile', size: 'large', glyph: ICONS.move(28), run: command('file.move-file') },
        {
          id: 'autoSave',
          size: 'large',
          glyph: ICONS.autoSave(28),
          run: command('file.toggle-auto-save'),
          isActive: () => autoSave.value
        }
      ]
    },
    {
      id: 'export',
      layout: 'row',
      items: [
        { id: 'exportDocx', size: 'large', glyph: ICONS.word(28), run: exportAs('docx') },
        { id: 'exportPdf', size: 'large', glyph: ICONS.pdf(28), run: exportAs('pdf') },
        { id: 'exportHtml', size: 'large', glyph: ICONS.htmlExport(28), run: exportAs('styledHtml') },
        { id: 'print', size: 'large', glyph: ICONS.print(28), run: exportAs('print') }
      ]
    },
    {
      id: 'close',
      layout: 'row',
      items: [
        { id: 'closeTab', size: 'large', glyph: ICONS.closeTab(28), run: command('file.close-tab') },
        { id: 'closeWindow', size: 'large', glyph: ICONS.closeWindow(28), run: command('file.close-window') },
        { id: 'preferences', size: 'large', glyph: ICONS.settings(28), run: command('file.preferences') },
        { id: 'quit', size: 'large', glyph: ICONS.quit(28), run: command('file.quit') }
      ]
    }
  ],
  home: [
    {
      id: 'clipboard',
      layout: 'grid',
      items: [
        { id: 'paste', glyph: ICONS.paste(), showLabel: true, run: editRole('paste') },
        { id: 'pasteAsPlainText', glyph: ICONS.pastePlain(), run: () => bus.emit('pasteAsPlainText', 'pasteAsPlainText') },
        { id: 'cut', glyph: ICONS.cut(), showLabel: true, run: editRole('cut') },
        { id: 'copyAsRich', glyph: ICONS.copyRich(), run: () => bus.emit('copyAsRich', 'copyAsRich') },
        { id: 'copy', glyph: ICONS.copy(), showLabel: true, run: editRole('copy') },
        { id: 'copyAsHtml', glyph: ICONS.copyHtml(), run: () => bus.emit('copyAsHtml', 'copyAsHtml') }
      ]
    },
    {
      id: 'undo',
      layout: 'column',
      items: [
        { id: 'undo', glyph: ICONS.undo, run: () => bus.emit('undo') },
        { id: 'redo', glyph: ICONS.redo, run: () => bus.emit('redo') }
      ]
    },
    {
      id: 'font',
      layout: 'grid',
      items: [
        { id: 'bold', glyph: '<b>B</b>', run: format('strong') },
        { id: 'italic', glyph: '<i>I</i>', run: format('em') },
        { id: 'underline', glyph: '<u>U</u>', run: format('u') },
        { id: 'strikethrough', glyph: '<s>ab</s>', run: format('del') },
        { id: 'subscript', glyph: 'x<sub>2</sub>', run: format('sub') },
        { id: 'superscript', glyph: 'x<sup>2</sup>', run: format('sup') },
        { id: 'highlight', glyph: '<span class="hl">ab</span>', run: format('mark') },
        { id: 'inlineCode', glyph: ICONS.code, run: format('inline_code') },
        { id: 'inlineMath', glyph: '∑', run: format('inline_math') },
        { id: 'clearFormat', glyph: ICONS.clear, run: format('clear') }
      ]
    },
    {
      id: 'paragraph',
      layout: 'grid',
      items: [
        { id: 'bulletList', glyph: ICONS.bullets, run: paragraph('ul-bullet') },
        { id: 'orderedList', glyph: ICONS.numbers, run: paragraph('ol-order') },
        { id: 'taskList', glyph: ICONS.tasks, run: paragraph('ul-task') },
        { id: 'decreaseHeading', glyph: 'A<sup>−</sup>', run: paragraph('degrade heading') },
        { id: 'increaseHeading', glyph: 'A<sup>+</sup>', run: paragraph('upgrade heading') },
        { id: 'quote', glyph: ICONS.quote(), run: paragraph('blockquote') },
        { id: 'codeBlock', glyph: ICONS.codeBlock(), run: paragraph('pre') },
        { id: 'horizontalLine', glyph: ICONS.hr(), run: paragraph('hr') }
      ]
    },
    { id: 'styles', items: [] },
    {
      id: 'editing',
      layout: 'grid',
      items: [
        { id: 'find', glyph: ICONS.find(), showLabel: true, run: () => bus.emit('mt::editor-edit-action', 'find') },
        { id: 'replace', glyph: ICONS.replace(), showLabel: true, run: () => bus.emit('mt::editor-edit-action', 'replace') },
        { id: 'findInFolder', glyph: ICONS.folderSearch(), showLabel: true, run: () => bus.emit('mt::editor-edit-action', 'findInFolder') },
        { id: 'selectAll', glyph: ICONS.selectAll(), showLabel: true, run: () => bus.emit('selectAll', 'selectAll') }
      ]
    },
    {
      id: 'blockActions',
      layout: 'column',
      items: [
        { id: 'duplicate', glyph: ICONS.duplicate(), showLabel: true, run: () => bus.emit('duplicate', 'duplicate') },
        { id: 'createParagraph', glyph: ICONS.newPara(), showLabel: true, run: () => bus.emit('createParagraph', 'createParagraph') },
        { id: 'deleteParagraph', glyph: ICONS.deletePara(), showLabel: true, run: () => bus.emit('deleteParagraph', 'deleteParagraph') }
      ]
    }
  ],
  insert: [
    {
      id: 'tables',
      layout: 'row',
      items: [{ id: 'table', size: 'large', glyph: ICONS.table(28), run: paragraph('table') }]
    },
    {
      id: 'illustrations',
      layout: 'row',
      items: [
        { id: 'image', size: 'large', glyph: ICONS.image(28), run: format('image') },
        { id: 'link', size: 'large', glyph: ICONS.link(28), run: format('link') }
      ]
    },
    {
      id: 'blocks',
      layout: 'row',
      items: [
        { id: 'codeBlock', size: 'large', glyph: ICONS.codeBlock(28), run: paragraph('pre') },
        { id: 'mathBlock', size: 'large', glyph: ICONS.math(28), run: paragraph('mathblock') },
        { id: 'quote', size: 'large', glyph: ICONS.quote(28), run: paragraph('blockquote') },
        { id: 'horizontalLine', size: 'large', glyph: ICONS.hr(28), run: paragraph('hr') },
        { id: 'htmlBlock', size: 'large', glyph: ICONS.html(28), run: paragraph('html') },
        { id: 'frontMatter', size: 'large', glyph: ICONS.frontMatter(28), run: paragraph('front-matter') }
      ]
    },
    {
      id: 'more',
      layout: 'grid',
      items: [
        { id: 'heading5', glyph: 'H5', run: paragraph('heading 5') },
        { id: 'heading6', glyph: 'H6', run: paragraph('heading 6') },
        { id: 'looseListItem', glyph: ICONS.bullets, run: paragraph('loose-list-item') },
        { id: 'screenshot', glyph: ICONS.screenshot(), run: command('edit.screenshot') }
      ]
    }
  ],
  design: [
    { id: 'themes', items: [] },
    {
      id: 'themeOptions',
      layout: 'column',
      items: [
        {
          id: 'followSystemTheme',
          glyph: ICONS.contrast(),
          showLabel: true,
          run: () =>
            window.electron.ipcRenderer.send('mt::set-user-preference', {
              followSystemTheme: !followSystemTheme.value
            }),
          isActive: () => followSystemTheme.value
        },
        { id: 'customizeTheme', glyph: ICONS.settings(), showLabel: true, run: command('file.preferences') }
      ]
    }
  ],
  layout: [
    {
      id: 'pageSize',
      layout: 'row',
      items: (['a4', 'letter', 'legal', 'none'] as const).map((size) => ({
        id: `size_${size}`,
        size: 'large' as const,
        glyph: size === 'none' ? ICONS.web(28) : ICONS.page(28),
        run: () => setPref('pageLayout', size),
        isActive: () => pageLayout.value === size
      }))
    },
    {
      id: 'margins',
      layout: 'row',
      items: MARGINS.map((margin) => ({
        id: margin.id,
        size: 'large' as const,
        glyph: ICONS.margins(28),
        run: () => {
          if (pageLayout.value === 'none') setPref('pageLayout', 'a4')
          setPref('pageMargin', margin.mm)
        },
        isActive: () => pageLayout.value !== 'none' && Math.abs(pageMargin.value - margin.mm) < 0.01
      }))
    },
    {
      id: 'export',
      layout: 'row',
      items: [
        { id: 'exportDocx', size: 'large', glyph: ICONS.word(28), run: exportAs('docx') },
        { id: 'exportPdf', size: 'large', glyph: ICONS.pdf(28), run: exportAs('pdf') },
        { id: 'exportHtml', size: 'large', glyph: ICONS.htmlExport(28), run: exportAs('styledHtml') }
      ]
    }
  ],
  view: [
    {
      id: 'views',
      layout: 'row',
      items: [
        {
          id: 'printLayout',
          size: 'large',
          glyph: ICONS.page(28),
          run: () => {
            if (sourceCode.value) bus.emit('view:toggle-view-entry', 'sourceCode')
            if (pageLayout.value === 'none') setPref('pageLayout', 'a4')
          },
          isActive: () => !sourceCode.value && pageLayout.value !== 'none'
        },
        {
          id: 'webLayout',
          size: 'large',
          glyph: ICONS.web(28),
          run: () => {
            if (sourceCode.value) bus.emit('view:toggle-view-entry', 'sourceCode')
            setPref('pageLayout', 'none')
          },
          isActive: () => !sourceCode.value && pageLayout.value === 'none'
        },
        {
          id: 'sourceCode',
          size: 'large',
          glyph: ICONS.source(28),
          run: toggleView('sourceCode'),
          isActive: () => sourceCode.value
        }
      ]
    },
    {
      id: 'immersive',
      layout: 'row',
      items: [
        { id: 'focusMode', size: 'large', glyph: ICONS.focus(28), run: toggleView('focus'), isActive: () => focus.value },
        {
          id: 'typewriterMode',
          size: 'large',
          glyph: ICONS.typewriter(28),
          run: toggleView('typewriter'),
          isActive: () => typewriter.value
        }
      ]
    },
    {
      id: 'show',
      layout: 'row',
      items: [
        { id: 'sideBar', size: 'large', glyph: ICONS.sidebar(28), run: toggleLayout('showSideBar'), isActive: () => showSideBar.value },
        { id: 'tabBar', size: 'large', glyph: ICONS.tabsIcon(28), run: toggleLayout('showTabBar'), isActive: () => showTabBar.value }
      ]
    },
    {
      id: 'zoom',
      layout: 'row',
      items: [
        { id: 'zoomIn', size: 'large', glyph: ICONS.zoomIn(28), run: stepZoom(1) },
        { id: 'zoomOut', size: 'large', glyph: ICONS.zoomOut(28), run: stepZoom(-1) },
        { id: 'zoomReset', size: 'large', glyph: ICONS.zoomReset(28), run: () => bus.emit('mt::window-zoom', 1) }
      ]
    },
    {
      id: 'window',
      layout: 'row',
      items: [
        { id: 'fullScreen', size: 'large', glyph: ICONS.fullScreen(28), run: command('window.toggle-full-screen') },
        { id: 'alwaysOnTop', size: 'large', glyph: ICONS.pin(28), run: command('window.toggle-always-on-top') },
        { id: 'minimize', size: 'large', glyph: ICONS.minimize(28), run: command('window.minimize') },
        { id: 'textLtr', size: 'large', glyph: ICONS.ltr(28), run: () => setPref('textDirection', 'ltr') },
        { id: 'textRtl', size: 'large', glyph: ICONS.rtl(28), run: () => setPref('textDirection', 'rtl') }
      ]
    }
  ],
  help: [
    {
      id: 'learn',
      layout: 'row',
      items: [
        { id: 'userGuide', size: 'large', glyph: ICONS.book(28), run: command('docs.user-guide') },
        { id: 'markdownSyntax', size: 'large', glyph: ICONS.markdown(28), run: command('docs.markdown-syntax') },
        { id: 'commandPalette', size: 'large', glyph: ICONS.palette(28), run: () => bus.emit('show-command-palette') }
      ]
    },
    {
      id: 'about',
      layout: 'row',
      items: [
        { id: 'checkUpdates', size: 'large', glyph: ICONS.update(28), run: () => window.electron.ipcRenderer.send('mt::check-for-update') },
        { id: 'aboutMarkd', size: 'large', glyph: ICONS.info(28), run: () => bus.emit('aboutDialog') }
      ]
    }
  ]
}

const currentGroups = computed(() => GROUPS[activeTab.value])

// Narrow windows: let the mouse wheel scroll the ribbon sideways.
const scrollSideways = (event: WheelEvent): void => {
  const el = event.currentTarget as HTMLElement
  if (el.scrollWidth <= el.clientWidth || Math.abs(event.deltaX) > Math.abs(event.deltaY)) return
  el.scrollLeft += event.deltaY
  event.preventDefault()
}
</script>

<style scoped>
.ribbon {
  flex-shrink: 0;
  background: var(--wordRibbonBg, var(--sideBarBgColor, var(--editorBgColor)));
  border-bottom: 1px solid var(--editorColor10);
  user-select: none;
  color: var(--editorColor);
  font-size: 12px;
}

.ribbon-tabs {
  display: flex;
  align-items: flex-end;
  gap: 2px;
  padding: 0 10px;
  height: 30px;
}

.ribbon-tab {
  position: relative;
  height: 28px;
  padding: 0 12px;
  border: none;
  background: transparent;
  color: var(--editorColor80);
  font-size: 13px;
  cursor: pointer;
  border-radius: 4px 4px 0 0;
}

.ribbon-tab:hover {
  background: var(--editorColor04);
  color: var(--editorColor);
}

.ribbon-tab.active {
  color: var(--wordAccent, var(--themeColor));
  font-weight: 600;
}

.ribbon-tab.active::after {
  content: '';
  position: absolute;
  left: 12px;
  right: 12px;
  bottom: 0;
  height: 3px;
  border-radius: 2px;
  background: var(--wordAccent, var(--themeColor));
}

.ribbon-collapse {
  margin-left: auto;
  align-self: center;
  width: 26px;
  height: 22px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--editorColor60);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.ribbon-collapse:hover {
  background: var(--editorColor10);
}

.ribbon-body {
  display: flex;
  align-items: stretch;
  min-height: 86px;
  padding: 4px 6px 2px;
  margin: 0 6px 6px;
  border-radius: 6px;
  background: var(--editorBgColor);
  box-shadow: 0 0 0 1px var(--editorColor10), 0 1px 2px rgba(0, 0, 0, 0.06);
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: thin;
}

.ribbon-body::-webkit-scrollbar,
.theme-gallery::-webkit-scrollbar {
  height: 4px;
  background: transparent;
}

.ribbon-body::-webkit-scrollbar-thumb,
.theme-gallery::-webkit-scrollbar-thumb {
  border-radius: 2px;
  background: transparent;
}

.ribbon-body:hover::-webkit-scrollbar-thumb,
.theme-gallery:hover::-webkit-scrollbar-thumb {
  background: var(--editorColor30);
}

.ribbon-group {
  display: flex;
  flex-direction: column;
  padding: 0 8px;
  border-right: 1px solid var(--editorColor10);
  flex-shrink: 0;
}

.ribbon-group:last-child {
  border-right: none;
}

.group-items {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 2px;
}

.group-items.grid {
  display: grid;
  grid-template-rows: repeat(2, 28px);
  grid-auto-flow: column;
  align-content: center;
}

.group-items.column {
  flex-direction: column;
  justify-content: center;
  align-items: stretch;
}

.group-caption {
  text-align: center;
  font-size: 11px;
  color: var(--editorColor50);
  padding: 2px 0 1px;
  white-space: nowrap;
}

.tool {
  border: 1px solid transparent;
  border-radius: 4px;
  background: transparent;
  color: var(--editorColor80);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  font-size: 14px;
  line-height: 1;
}

.tool.small {
  min-width: 28px;
  height: 28px;
  padding: 0 6px;
}

.tool.large {
  flex-direction: column;
  min-width: 56px;
  height: 64px;
  padding: 4px 6px;
  gap: 4px;
}

.tool.large .label {
  font-size: 11px;
  max-width: 72px;
  white-space: normal;
  text-align: center;
  line-height: 1.15;
}

.tool .label {
  font-size: 12px;
}

.tool:hover {
  background: var(--editorColor04);
  border-color: var(--editorColor10);
  color: var(--editorColor);
}

.tool.active {
  background: var(--themeColor10);
  border-color: var(--themeColor30);
  color: var(--wordAccent, var(--themeColor));
}

.tool:active {
  background: var(--themeColor20);
}

.glyph {
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.glyph :deep(svg) {
  display: block;
}

.glyph :deep(.hl) {
  background: #ffeb3b;
  color: #262626;
  padding: 0 2px;
}

.glyph :deep(sup),
.glyph :deep(sub) {
  font-size: 9px;
}

.style-tile {
  width: 78px;
  height: 64px;
  margin: 0 1px;
  border: 1px solid var(--editorColor10);
  border-radius: 4px;
  background: var(--editorBgColor);
  color: var(--editorColor);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  overflow: hidden;
}

.style-tile:hover {
  border-color: var(--wordAccent, var(--themeColor));
}

.style-preview {
  font-size: 13px;
  white-space: nowrap;
}

.style-name {
  font-size: 11px;
  color: var(--editorColor60);
}

.style-tile.heading1 .style-preview {
  font-size: 17px;
  color: var(--h1Color, var(--headingColor));
}

.style-tile.heading2 .style-preview {
  font-size: 15px;
  color: var(--h2Color, var(--headingColor));
}

.style-tile.heading3 .style-preview {
  font-size: 14px;
  font-weight: 600;
  color: var(--h3Color, var(--headingColor));
}

.style-tile.heading4 .style-preview {
  font-style: italic;
  color: var(--h4Color, var(--headingColor));
}

.style-tile.quote .style-preview {
  font-style: italic;
  color: var(--blockquoteTextColor, var(--editorColor60));
}

.theme-gallery {
  display: grid;
  grid-template-rows: repeat(2, 30px);
  grid-auto-flow: column;
  grid-auto-columns: 124px;
  gap: 4px;
  max-width: 920px;
  overflow-x: auto;
  padding: 2px;
}

.theme-tile {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 2px 6px 2px 2px;
  border: 1px solid var(--editorColor10);
  border-radius: 4px;
  background: var(--editorBgColor);
  color: var(--editorColor80);
  cursor: pointer;
  font-size: 11px;
  text-align: left;
  overflow: hidden;
}

.theme-tile:hover {
  border-color: var(--wordAccent, var(--themeColor));
}

.theme-tile.active {
  border-color: var(--wordAccent, var(--themeColor));
  box-shadow: inset 0 0 0 1px var(--wordAccent, var(--themeColor));
}

.theme-swatch {
  flex-shrink: 0;
  width: 26px;
  height: 24px;
  border-radius: 3px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  font-weight: 600;
  box-shadow: inset 0 0 0 1px rgba(127, 127, 127, 0.35);
}

.theme-name {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ribbon.collapsed {
  padding-bottom: 2px;
}
</style>
