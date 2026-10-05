// Tiny Markdown subset for changelog entries: "- " bullets, _italic_ notes and `code`.

export interface MdBlock {
  kind: 'bullet' | 'note' | 'text'
  html: string
}

function inline(text: string): string {
  const escaped = text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  return escaped.replace(/`([^`]+)`/g, '<code>$1</code>').replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
}

export function renderBlocks(markdown: string): MdBlock[] {
  return markdown
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line): MdBlock => {
      if (line.startsWith('- ') || line.startsWith('* '))
        return { kind: 'bullet', html: inline(line.slice(2)) }
      if (/^_.*_$/.test(line)) return { kind: 'note', html: inline(line.slice(1, -1)) }
      return { kind: 'text', html: inline(line) }
    })
}
