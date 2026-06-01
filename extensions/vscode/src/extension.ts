import * as vscode from 'vscode';
import * as path from 'path';
import { GlanceDaemon, OpenedData } from './daemon';
import { openFilePanel, setupPanel } from './panel';

// Files >= 25MB open in Glance. Below this, VS Code text editor is fine.
const THRESHOLD_MB = 25;
const THRESHOLD    = THRESHOLD_MB * 1024 * 1024;

let daemon: GlanceDaemon | undefined;

export function activate(context: vscode.ExtensionContext) {
  const binaryName = process.platform === 'win32' ? 'glance.exe' : 'glance';
  const daemonPath = path.join(context.extensionPath, 'bin', binaryName);
  daemon = new GlanceDaemon(daemonPath);

  hideIndexFiles();

  // Register custom editor — "priority: default" means Glance handles these file types.
  // Size check happens inside resolveCustomEditor, not via redirects.
  context.subscriptions.push(
    vscode.window.registerCustomEditorProvider(
      'glance.fileViewer',
      new GlanceEditorProvider(context, daemon),
      { supportsMultipleEditorsPerDocument: false }
    )
  );

  // Manual right-click override
  context.subscriptions.push(
    vscode.commands.registerCommand('glance.openFile', async (uri?: vscode.Uri) => {
      if (!uri) {
        const picked = await vscode.window.showOpenDialog({
          filters: {
            'Large Files': ['jsonl', 'ndjson', 'ldjson', 'csv', 'tsv', 'log', 'sql', 'txt', 'xml', 'out'],
          },
        });
        if (!picked?.[0]) { return; }
        uri = picked[0];
      }
      await vscode.commands.executeCommand('vscode.openWith', uri, 'glance.fileViewer');
    })
  );
}

export function deactivate() {
  daemon?.dispose();
}

// ── CustomReadonlyEditorProvider ───────────────────────────────────────────────

class GlanceDocument implements vscode.CustomDocument {
  constructor(public readonly uri: vscode.Uri) {}
  dispose(): void {}
}

class GlanceEditorProvider implements vscode.CustomReadonlyEditorProvider<GlanceDocument> {
  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly daemon: GlanceDaemon
  ) {}

  openCustomDocument(uri: vscode.Uri): GlanceDocument {
    return new GlanceDocument(uri);
  }

  async resolveCustomEditor(
    document: GlanceDocument,
    webviewPanel: vscode.WebviewPanel
  ): Promise<void> {
    // Check file size FIRST — before doing anything with the webview.
    // This is the single source of truth for the size decision.
    let fileSize = 0;
    try {
      const stat = await vscode.workspace.fs.stat(document.uri);
      fileSize = stat.size;
    } catch (_) { /* unknown size — proceed with Glance */ }

    if (fileSize > 0 && fileSize < THRESHOLD) {
      // Small file — dispose this panel and open with VS Code text editor instead.
      // setTimeout gives VS Code a tick to finish registering the panel before disposing.
      setTimeout(() => {
        webviewPanel.dispose();
        vscode.commands.executeCommand('vscode.openWith', document.uri, 'default');
      }, 0);
      return;
    }

    // Large file (or size unknown) — open in Glance.
    webviewPanel.webview.options = {
      enableScripts: true,
      localResourceRoots: [vscode.Uri.joinPath(this.context.extensionUri, 'media')],
    };

    let info: OpenedData;
    try {
      info = await this.daemon.call<OpenedData>('open', { path: document.uri.fsPath });
    } catch (err: unknown) {
      webviewPanel.webview.html = `<body style="font-family:sans-serif;color:var(--vscode-errorForeground);padding:20px">
        <b>Glance error:</b> ${(err as Error).message}</body>`;
      return;
    }

    setupPanel(webviewPanel.webview, webviewPanel, this.context.extensionUri, this.daemon, info);
  }
}

// ── Utilities ──────────────────────────────────────────────────────────────────

async function hideIndexFiles() {
  try {
    const config  = vscode.workspace.getConfiguration('files');
    const exclude = { ...(config.get<Record<string, boolean>>('exclude') ?? {}) };
    if (!exclude['**/*.glance_idx']) {
      exclude['**/*.glance_idx'] = true;
      await config.update('exclude', exclude, vscode.ConfigurationTarget.Workspace);
    }
  } catch (_) { /* non-critical */ }
}
