import type { SiteConfig } from '@mcptoolshop/site-theme';

export const config: SiteConfig = {
  title: 'CommandUI',
  description: 'AI-native shell environment with semantic command review.',
  logoBadge: 'C',
  brandName: 'CommandUI',
  repoUrl: 'https://github.com/mcp-tool-shop-org/commandui',
  footerText: 'MIT Licensed — built by <a href="https://github.com/mcp-tool-shop-org" style="color:var(--color-muted);text-decoration:underline">mcp-tool-shop-org</a>',

  hero: {
    badge: 'Desktop app',
    headline: 'CommandUI',
    headlineAccent: 'AI-native shell.',
    description: 'Real terminal. Semantic input. You review every command before it runs.',
    primaryCta: { href: '#usage', label: 'Get started' },
    secondaryCta: { href: 'handbook/', label: 'Read the Handbook' },
    previews: [
      { label: 'Command', code: 'git status --short' },
      { label: 'Ask', code: '"show me changed files"  →  git status --short' },
      { label: 'Workflow', code: 'git add → git commit → git push' },
    ],
  },

  sections: [
    {
      kind: 'features',
      id: 'features',
      title: 'Features',
      subtitle: 'Terminal power without terminal hostility.',
      features: [
        {
          title: 'Real Shell',
          desc: 'Full PTY sessions with stdout/stderr streaming, exit codes, cwd tracking, and multi-session tabs. Not a wrapper — a real terminal.',
        },
        {
          title: 'Semantic Input',
          desc: 'Describe intent in natural language. The AI planner generates a shell command with explanation, risk assessment, and assumptions. You review before anything runs.',
        },
        {
          title: 'Risk-Tiered Safety',
          desc: 'Low-risk commands flow. Medium-risk commands ask. High-risk commands require explicit confirmation. Nothing destructive executes without your approval.',
        },
        {
          title: 'Edit Before Run',
          desc: 'Every generated command is editable. Modify it, add flags, change paths — then approve. History records both the original and your edit.',
        },
        {
          title: 'Workflow Promotion',
          desc: 'Repeat a command sequence three times and the system suggests saving it as a workflow. Promoted workflows feed back into the planner for better future suggestions.',
        },
        {
          title: 'Project Memory',
          desc: 'Learns your preferred tools, directories, and command patterns. Confidence-scored, visible, editable, deletable. Feeds the planner so it stops acting like a stranger.',
        },
      ],
    },
    {
      kind: 'code-cards',
      id: 'usage',
      title: 'Quick Start',
      cards: [
        {
          title: 'Clone & install',
          code: 'git clone https://github.com/mcp-tool-shop-org/commandui.git\ncd commandui\npnpm install',
        },
        {
          title: 'Browser preview',
          code: 'pnpm dev\n# Opens at http://localhost:5176\n# Mock bridge simulates all backend ops',
        },
        {
          title: 'Full desktop app',
          code: 'cd apps/desktop\npnpm tauri:dev\n# Rust backend + real PTY shell',
        },
        {
          title: 'Run tests',
          code: 'pnpm typecheck\npnpm test\ncd apps/desktop/src-tauri && cargo test',
        },
      ],
    },
    {
      kind: 'features',
      id: 'architecture',
      title: 'Architecture',
      subtitle: 'Six layers, clear boundaries, local-first.',
      features: [
        {
          title: 'Tauri v2 + React 19',
          desc: 'Rust backend for PTY management, SQLite persistence, and Ollama LLM integration. React frontend with Zustand state management and xterm.js terminal.',
        },
        {
          title: 'Monorepo Packages',
          desc: 'Domain types, API contracts, and state stores as separate packages. Dependencies flow downward. Frontend never touches Rust directly.',
        },
        {
          title: 'Ollama-First Planning',
          desc: 'Local LLM generates command plans with context-aware prompts. Falls back to mock planner when Ollama is unavailable. Zero cloud dependency.',
        },
      ],
    },
  ],
};
