<template>
  <div
    class="status-bar"
    role="status"
  >
    <div class="left">
      <span
        v-if="pageCount > 0"
        class="item"
      >{{ t('statusBar.page', { current: currentPage, total: pageCount }) }}</span>
      <span class="item">{{ t('statusBar.words', { count: words }) }}</span>
      <span class="item">{{ t('statusBar.characters', { count: characters }) }}</span>
      <span
        v-if="pageLabel"
        class="item"
      >{{ pageLabel }}</span>
    </div>
    <div class="right">
      <button
        type="button"
        class="view-button"
        :class="{ active: focus }"
        :title="t('formatToolbar.focusMode')"
        @click="bus.emit('view:toggle-view-entry', 'focus')"
      >
        <svg
          viewBox="0 0 24 24"
          width="14"
          height="14"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
        ><circle
          cx="12"
          cy="12"
          r="3"
        /><path d="M3 8V5a2 2 0 0 1 2-2h3M16 3h3a2 2 0 0 1 2 2v3M21 16v3a2 2 0 0 1-2 2h-3M8 21H5a2 2 0 0 1-2-2v-3" /></svg>
      </button>
      <span class="divider" />
      <button
        type="button"
        class="view-button"
        :class="{ active: !sourceCode && pageLayout !== 'none' }"
        :title="t('formatToolbar.printLayout')"
        @click="setView('print')"
      >
        <svg
          viewBox="0 0 24 24"
          width="14"
          height="14"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
        ><path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z" /><path d="M14 3v6h6" /></svg>
      </button>
      <button
        type="button"
        class="view-button"
        :class="{ active: !sourceCode && pageLayout === 'none' }"
        :title="t('formatToolbar.webLayout')"
        @click="setView('web')"
      >
        <svg
          viewBox="0 0 24 24"
          width="14"
          height="14"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
        ><rect
          x="3"
          y="4"
          width="18"
          height="16"
          rx="2"
        /><path d="M3 9h18" /></svg>
      </button>
      <button
        type="button"
        class="view-button"
        :class="{ active: sourceCode }"
        :title="t('formatToolbar.sourceCode')"
        @click="setView('source')"
      >
        <svg
          viewBox="0 0 24 24"
          width="14"
          height="14"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
        ><path d="m8 9-3 3 3 3" /><path d="m16 9 3 3-3 3" /></svg>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { storeToRefs } from 'pinia'
import bus from '@/bus'
import { usePreferencesStore } from '@/store/preferences'
import { useEditorStore } from '@/store/editor'
import { getPageDimensions } from 'common/pageLayout'

const { t } = useI18n()
const preferencesStore = usePreferencesStore()
const { pageLayout, sourceCode, focus } = storeToRefs(preferencesStore)
const { currentFile } = storeToRefs(useEditorStore())

const words = computed(() => currentFile.value?.wordCount?.word ?? 0)
const characters = computed(() => currentFile.value?.wordCount?.character ?? 0)
const pageLabel = computed(() => {
  const page = getPageDimensions(pageLayout.value)
  return page && !sourceCode.value ? t(`preferences.editor.textEditor.pageLayoutOptions.${pageLayout.value}`) : ''
})

const MM_TO_PX = 96 / 25.4
const pageCount = ref(0)
const currentPage = ref(1)

let scroller: HTMLElement | null = null
let resizeObserver: ResizeObserver | null = null
let frame = 0

const measure = (): void => {
  frame = 0
  const page = getPageDimensions(pageLayout.value)
  const sheet = document.querySelector<HTMLElement>('.editor-component .mu-container')
  if (!page || !sheet || sourceCode.value) {
    pageCount.value = 0
    return
  }
  const pageHeight = page.height * MM_TO_PX
  pageCount.value = Math.max(1, Math.ceil((sheet.offsetHeight - 1) / pageHeight))
  if (scroller) {
    const offset = scroller.scrollTop + scroller.clientHeight / 3 - sheet.offsetTop
    currentPage.value = Math.min(pageCount.value, Math.max(1, Math.floor(offset / pageHeight) + 1))
  }
}

const schedule = (): void => {
  if (!frame) frame = requestAnimationFrame(measure)
}

const attach = (): void => {
  const next = document.querySelector<HTMLElement>('.editor-component')
  if (next === scroller) return
  scroller?.removeEventListener('scroll', schedule)
  resizeObserver?.disconnect()
  scroller = next
  if (!scroller) return
  scroller.addEventListener('scroll', schedule, { passive: true })
  resizeObserver = new ResizeObserver(schedule)
  resizeObserver.observe(scroller)
  const sheet = scroller.querySelector('.mu-container')
  if (sheet) resizeObserver.observe(sheet)
  schedule()
}

const setView = (view: 'print' | 'web' | 'source'): void => {
  if (view === 'source') {
    if (!sourceCode.value) bus.emit('view:toggle-view-entry', 'sourceCode')
    return
  }
  if (sourceCode.value) bus.emit('view:toggle-view-entry', 'sourceCode')
  if (view === 'web') {
    preferencesStore.SET_SINGLE_PREFERENCE({ type: 'pageLayout', value: 'none' })
  } else if (pageLayout.value === 'none') {
    preferencesStore.SET_SINGLE_PREFERENCE({ type: 'pageLayout', value: 'a4' })
  }
}

let poll: ReturnType<typeof setInterval> | null = null
onMounted(() => {
  attach()
  // The editor mounts asynchronously; re-attach until it exists and whenever
  // the word count changes (cheap, rAF-coalesced).
  poll = setInterval(attach, 1000)
})

watch([pageLayout, sourceCode, words], () => {
  attach()
  schedule()
})

onBeforeUnmount(() => {
  if (poll) clearInterval(poll)
  if (frame) cancelAnimationFrame(frame)
  scroller?.removeEventListener('scroll', schedule)
  resizeObserver?.disconnect()
})
</script>

<style scoped>
.status-bar {
  flex-shrink: 0;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px;
  font-size: 12px;
  color: var(--editorColor60);
  background: var(--wordRibbonBg, var(--sideBarBgColor, var(--editorBgColor)));
  border-top: 1px solid var(--editorColor10);
  user-select: none;
}

.left,
.right {
  display: flex;
  align-items: center;
  gap: 2px;
}

.item {
  padding: 0 8px;
  white-space: nowrap;
}

.item + .item {
  border-left: 1px solid var(--editorColor10);
}

.view-button {
  width: 26px;
  height: 20px;
  border: none;
  border-radius: 3px;
  background: transparent;
  color: var(--editorColor60);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.view-button:hover {
  background: var(--editorColor10);
  color: var(--editorColor);
}

.view-button.active {
  background: var(--themeColor20);
  color: var(--wordAccent, var(--themeColor));
}

.divider {
  width: 1px;
  height: 14px;
  margin: 0 4px;
  background: var(--editorColor10);
}
</style>
