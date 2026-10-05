import type { SiteConfig } from '@mcptoolshop/site-theme';

export const config: SiteConfig = {
  title: 'CommandUI',
  description: 'A shell for people the terminal shuts out. It explains every result in plain words and never runs a drafted command until you approve it.',
  logoBadge: 'C',
  brandName: 'CommandUI',
  repoUrl: 'https://github.com/mcp-tool-shop-org/commandui',
  footerText: 'MIT Licensed — built by <a href="https://mcp-tool-shop.github.io/" style="color:var(--color-muted);text-decoration:underline">MCP Tool Shop</a>',

  hero: {
    badge: 'Windows desktop app',
    headline: 'CommandUI',
    headlineAccent: 'A shell you can follow.',
    description: 'Every result in a plain sentence. Ask in plain words, read the command, and nothing runs until you approve it. Built for the keyboard and for screen readers.',
    primaryCta: { href: '#install', label: 'Download' },
    secondaryCta: { href: 'handbook/', label: 'Read the Handbook' },
    previews: [
      { label: 'Command', code: 'git status --short' },
      { label: 'Ask', code: '"show me changed files"  →  git status --short' },
      { label: 'Workflow', code: 'git add → git commit → git push' },
    ],
  },

  sections: [
    {
      kind: 'code-cards',
      id: 'install',
      title: 'Install',
      cards: [
        {
          title: 'Microsoft Store',
          code: 'apps.microsoft.com/detail/9NTN1GFQJ91M\n\n# The Store has an earlier version until this\n# update is published there.',
        },
        {
          title: 'winget',
          code: 'winget install mcp-tool-shop.CommandUI\n\n# Installs v1.0.0 from GitHub Releases.',
        },
        {
          title: 'For Ask',
          code: '# Ask uses a model on your computer:\nollama pull qwen2.5:14b\n\n# Everything else works without it.',
        },
      ],
    },
    {
      kind: 'features',
      id: 'features',
      title: 'Features',
      subtitle: 'A real shell that tells you what happened.',
      features: [
        {
          title: 'Every result in a sentence',
          desc: '"Finished. 3 lines of output." or "Did not work. A file or folder in that command is not there." A failure offers Ask how to fix it and Run again.',
        },
        {
          title: 'Ask',
          desc: 'Describe what you want. CommandUI drafts a command, explains it in plain words, and waits. Run Plan is the approval.',
        },
        {
          title: 'A careful yes',
          desc: 'Low and medium risk need Run Plan. A command that deletes files, or that needs higher permissions, also waits until you type the folder name.',
        },
        {
          title: 'Edit Before Run',
          desc: 'Every generated command is editable. Modify it, add flags, change paths — then approve. History records both the original and your edit.',
        },
        {
          title: 'Workflows',
          desc: 'Make a list of commands, edit it, and run it again. History can save the commands you pick. A repeated sequence can be offered as a suggestion.',
        },
        {
          title: 'Memory',
          desc: 'Preferences CommandUI has noticed. You can read them and delete them.',
        },
      ],
    },
    {
      kind: 'features',
      id: 'accessibility',
      title: 'Built for the keyboard and screen readers',
      subtitle: 'Narrator, NVDA, and contrast-theme testing by people comes in a later update.',
      features: [
        {
          title: 'Announced, not just shown',
          desc: 'Results and errors are announced once, without moving your focus. Output (Ctrl+Shift+O) lists each command as plain text, with no terminal codes.',
        },
        {
          title: 'Every action by keyboard',
          desc: 'F1 opens keyboard help. Ctrl+Shift+R jumps to the last result. Ctrl+Shift+A switches between Command and Ask. Dialogs keep focus, and Escape returns it.',
        },
        {
          title: 'Larger text, your contrast',
          desc: 'Text from 100% to 200%, panels you can hide, and Windows contrast themes and reduced motion respected.',
        },
      ],
    },
    {
      kind: 'code-cards',
      id: 'usage',
      title: 'Build it',
      cards: [
        {
          title: 'Clone & install',
          code: 'git clone https://github.com/mcp-tool-shop-org/commandui.git\ncd commandui\npnpm install',
        },
        {
          title: 'Browser preview',
          code: 'pnpm dev\n# Opens the local address Vite prints.\n# Does not run your shell. A practice plan is labeled as practice.',
        },
        {
          title: 'Desktop app, while developing',
          code: 'cd apps/desktop\npnpm tauri:dev\n# Builds the desktop app and opens your shell.',
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
          title: 'Desktop app',
          desc: 'The window is React. The shell, the saved history, and Ask live in Rust. A second front end in this repo is not in the Store package.',
        },
        {
          title: 'Separate packages',
          desc: 'Types, contracts, and saved state are separate packages. The window does not reach into the shell code.',
        },
        {
          title: 'A model on this computer',
          desc: 'Ask uses a local model. If it is not installed, not running, or not downloaded, Ask says so and does not draft a command.',
        },
      ],
    },
  ],
};
