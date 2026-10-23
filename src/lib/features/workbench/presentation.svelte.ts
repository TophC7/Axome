import { sessionBrand, type HarnessIconSource } from '$lib/features/sessions/harnesses/catalog'
import type { Tab } from '$lib/features/workbench/model'
import { getDiffTabTitle } from '$lib/features/workbench/surfaces/diff/service.svelte'
import { getEpicTabTitle } from '$lib/features/workbench/surfaces/epic/service.svelte'
import { getIssueTabTitle } from '$lib/features/workbench/surfaces/issue/service.svelte'
import { getTaskTabIcon, getTaskTabTitle } from '$lib/features/workbench/surfaces/task/service.svelte'
import { getTextTabTitle } from '$lib/features/workbench/surfaces/text/service.svelte'

export interface TabPresentation {
  title: string
  preview: boolean
  /** Session harness icon (rendered via HarnessIcon). */
  sessionIcon: HarnessIconSource | null
  /** Kebab-case Lucide icon name for task tabs (rendered via LucideIcon). */
  lucideIcon: string | null
  fileName: string | null
}

export function getTabPresentation(tab: Tab): TabPresentation {
  const plain = { sessionIcon: null, lucideIcon: null, fileName: null }

  switch (tab.kind) {
    case 'new-tab':
      return { ...plain, title: 'New', preview: true }
    case 'session':
      return {
        ...plain,
        title: tab.title,
        preview: false,
        sessionIcon: sessionBrand(tab.folderPath, tab.harnessId).icon
      }
    case 'chat':
      return { ...plain, title: tab.title, preview: false }
    case 'shell':
      return { ...plain, title: tab.title, preview: false }
    case 'text':
      return { ...plain, title: getTextTabTitle(tab), preview: tab.temporary, fileName: tab.fileName }
    case 'diff':
      return { ...plain, title: getDiffTabTitle(tab), preview: tab.temporary }
    case 'task':
      return { ...plain, title: getTaskTabTitle(tab), preview: false, lucideIcon: getTaskTabIcon(tab) }
    case 'issue':
      return { ...plain, title: getIssueTabTitle(tab), preview: tab.temporary }
    case 'epic':
      return { ...plain, title: getEpicTabTitle(tab), preview: tab.temporary }
  }
}
