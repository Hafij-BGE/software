import { test, expect } from '@playwright/test'
import * as fs from 'node:fs'
import * as os from 'node:os'
import * as path from 'node:path'
import JSZip from 'jszip'
import { launchElectron } from './helpers'

// Markd additions: A4 page layout, formatting toolbar, extra themes and the
// themed native DOCX / PDF export.

const SAMPLE = `# Markd export test

Some **bold**, *italic*, ~~strike~~ and \`inline code\` text with a [link](https://example.com).

> A quote that should keep the theme colours.

- one
- two

| A | B |
|---|---|
| 1 | 2 |

\`\`\`js
const answer = 42
\`\`\`
`

const makeTempDir = (): string => fs.mkdtempSync(path.join(os.tmpdir(), 'markd-e2e-'))

const stubSaveDialog = async(
  app: Awaited<ReturnType<typeof launchElectron>>['app'],
  filePath: string
): Promise<void> => {
  await app.evaluate(({ dialog }, target) => {
    dialog.showSaveDialog = (async() => ({ canceled: false, filePath: target })) as never
  }, filePath)
}

const openExportDialog = async(
  app: Awaited<ReturnType<typeof launchElectron>>['app'],
  type: string
): Promise<void> => {
  await app.evaluate(({ BrowserWindow }, exportType) => {
    BrowserWindow.getAllWindows()[0]!.webContents.send('mt::show-export-dialog', exportType)
  }, type)
}

const waitForFile = async(filePath: string, timeout = 20000): Promise<void> => {
  const start = Date.now()
  while (Date.now() - start < timeout) {
    if (fs.existsSync(filePath) && fs.statSync(filePath).size > 0) return
    await new Promise((resolve) => setTimeout(resolve, 200))
  }
  throw new Error(`File not written: ${filePath}`)
}

test.describe('Markd features', () => {
  test('page layout, toolbar and themed DOCX/PDF export', async() => {
    const dir = makeTempDir()
    const mdPath = path.join(dir, 'sample.md')
    fs.writeFileSync(mdPath, SAMPLE, 'utf-8')

    const { app, page } = await launchElectron([mdPath], {
      preferences: { theme: 'word', pageLayout: 'a4', pageMargin: 25.4 }
    })
    try {
      await page.waitForSelector('.mu-container h1')

      // A4 sheet: 210mm wide (96 dpi => ~793.7px).
      const width = await page.$eval('.editor-component .mu-container', (el) =>
        el.getBoundingClientRect().width
      )
      expect(Math.abs(width - (210 / 25.4) * 96)).toBeLessThan(2)

      // Toolbar is present and drives the editor.
      await expect(page.locator('.ribbon')).toBeVisible()

      // Every former menu lives in the ribbon, so the hamburger menu is gone.
      await expect(page.locator('.frameless-titlebar-menu')).toHaveCount(0)
      for (const tab of ['File', 'Home', 'Insert', 'Design', 'Layout', 'View', 'Help']) {
        await expect(page.locator('.ribbon-tab', { hasText: tab })).toBeVisible()
      }

      // Picking a theme in Design applies it.
      await page.locator('.ribbon-tab', { hasText: 'Design' }).click()
      await page.locator('.theme-tile', { hasText: 'Night Owl' }).click()
      await expect
        .poll(() =>
          page.evaluate(() =>
            getComputedStyle(document.documentElement).getPropertyValue('--editorBgColor').trim().toLowerCase()
          )
        )
        .toBe('#011627')
      await page.screenshot({ path: path.join(dir, 'editor.png') })

      // DOCX export with the editor theme.
      const docxPath = path.join(dir, 'sample.docx')
      await stubSaveDialog(app, docxPath)
      await openExportDialog(app, 'docx')
      await waitForFile(docxPath)
      const zip = await JSZip.loadAsync(fs.readFileSync(docxPath))
      const documentXml = await zip.file('word/document.xml')!.async('string')
      expect(documentXml).toContain('Markd export test')
      expect(documentXml).toContain('w:background w:color="011627"')
      // A4 page size in twips.
      expect(documentXml).toMatch(/<w:pgSz[^>]*w:w="11906"/)

      // PDF export with the editor theme.
      const pdfPath = path.join(dir, 'sample.pdf')
      await stubSaveDialog(app, pdfPath)
      await openExportDialog(app, 'pdf')
      await waitForFile(pdfPath)
      expect(fs.readFileSync(pdfPath).subarray(0, 4).toString()).toBe('%PDF')

      if (process.env.MARKD_E2E_KEEP) console.log('artifacts:', dir)
    } finally {
      await app.close()
      if (!process.env.MARKD_E2E_KEEP) fs.rmSync(dir, { recursive: true, force: true })
    }
  })
})
