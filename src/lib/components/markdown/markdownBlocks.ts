import { unified } from 'unified'
import remarkParse from 'remark-parse'
import remarkGfm from 'remark-gfm'

export function splitMarkdownBlocks(source: string): string[] {
  if (!source) return []

  const { children } = unified().use(remarkParse).use(remarkGfm).parse(source)
  if (children.some((child) => child.type === 'definition' || child.type === 'footnoteDefinition')) {
    return [source]
  }

  const starts = [0]
  for (let index = 1; index < children.length; index++) {
    const offset = children[index].position?.start.offset
    if (offset === undefined) return [source]
    starts.push(offset)
  }
  return starts.map((start, index) => source.slice(start, starts[index + 1] ?? source.length))
}
