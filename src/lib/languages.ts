// Display names and colours of languages and categories used in manifests.

const LANGUAGES: Record<string, { name: string; short: string; from: string; to: string }> = {
  node: { name: 'Node.js', short: 'JS', from: '#63c46b', to: '#2f9e5a' },
  javascript: { name: 'JavaScript', short: 'JS', from: '#f7d64a', to: '#e0a92e' },
  typescript: { name: 'TypeScript', short: 'TS', from: '#4f8ff7', to: '#2f63d6' },
  rust: { name: 'Rust', short: 'RS', from: '#f28b50', to: '#c9562a' },
  php: { name: 'PHP', short: 'PHP', from: '#8e97e8', to: '#5b63c9' },
  python: { name: 'Python', short: 'PY', from: '#4f9bdc', to: '#e8c24a' },
  go: { name: 'Go', short: 'GO', from: '#5ad1e6', to: '#1a9fc1' },
}

export function language(id: string): { name: string; short: string; from: string; to: string } {
  return LANGUAGES[id] ?? { name: id, short: id.slice(0, 2).toUpperCase(), from: '#a0a4b8', to: '#747a94' }
}
