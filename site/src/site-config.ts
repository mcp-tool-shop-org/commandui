import type { SiteConfig } from '@mcptoolshop/site-theme';

export const config: SiteConfig = {
  title: 'CommandUI',
  description: 'A shell that explains every result, and waits for you to approve a drafted command.',
  logoBadge: 'C',
  brandName: 'CommandUI',
  repoUrl: 'https://github.com/mcp-tool-shop-org/commandui',
  footerText: 'MIT Licensed — built by <a href="https://mcp-tool-shop.github.io/" style="color:var(--color-muted);text-decoration:underline">MCP Tool Shop</a>',

  hero: {
    badge: 'Desktop app',
    headline: 'CommandUI',
    headlineAccent: 'A shell you can follow.',
    description: 'Ask in plain words. Read the command. Nothing runs until you approve it.',
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
          title: 'Store package',
          code: 'mcp-tool-shop.CommandUI\nx64 MSIX. Partner Center signs the upload.\nThe unsigned file is not a double-click installer.',
        },
        {
          title: 'Install today',
          code: 'winget install mcp-tool-shop.CommandUI\n\n# The public release is still the MSI.',
        },
        {
          title: 'Scoop',
          code: 'scoop bucket add mcp-tool-shop https://github.com/mcp-tool-shop-org/scoop-bucket\nscoop install commandui',
        },
      ],
    },
    {
      kind: 'features',
      id: 'features',
      title: 'Features',
      subtitle: 'Terminal power without terminal hostility.',
      features: [
        {
          title: 'Real shell',
          desc: 'Your own shell, more than one session, and a result sentence for every command you run from the command box.',
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
          code: 'pnpm dev\n# Opens at http://localhost:5176\n# Does not run your shell. A practice plan is labeled as practice.',
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
