import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { GlanceDaemon, OpenedData, LinesData, SearchResultsData, CountData } from './daemon';

type WebviewMessage =
  | { type: 'read'; offset: number; limit: number; pretty: boolean }
  | { type: 'search'; query: string; useRegex: boolean }
  | { type: 'count'; query: string; useRegex: boolean; gen: number };

/// Open a file in a new Glance panel (right-click command).
export async function openFilePanel(
  context: vscode.ExtensionContext,
  daemon: GlanceDaemon,
  uri: vscode.Uri
): Promise<void> {
  const fileName = path.basename(uri.fsPath);

  let info: OpenedData;
  try {
    info = await daemon.call<OpenedData>('open', { path: uri.fsPath });
  } catch (err: unknown) {
    vscode.window.showErrorMessage(`Glance: ${(err as Error).message}`);
    return;
  }

  const panel = vscode.window.createWebviewPanel(
    'glance',
    `Glance: ${fileName}`,
    vscode.ViewColumn.One,
    {
      enableScripts: true,
      localResourceRoots: [vscode.Uri.joinPath(context.extensionUri, 'media')],
    }
  );

  setupPanel(panel.webview, panel, context.extensionUri, daemon, info, uri.fsPath);
}

/// Wire up a webview (either from createWebviewPanel or CustomEditorProvider).
export function setupPanel(
  webview: vscode.Webview,
  disposable: { onDidDispose: (cb: () => void) => vscode.Disposable },
  extensionUri: vscode.Uri,
  daemon: GlanceDaemon,
  info: OpenedData,
  sourcePath: string
): void {
  webview.html = buildHtml(webview, extensionUri, info, sourcePath);

  const messageSubscription = webview.onDidReceiveMessage(
    async (msg: WebviewMessage) => {
      try {
        if (msg.type === 'read') {
          const data = await daemon.call<LinesData>('read', {
            file_id: info.file_id,
            offset: msg.offset,
            limit: msg.limit,
            pretty: msg.pretty,
          });
          webview.postMessage({ type: 'lines', ...data });

        } else if (msg.type === 'search') {
          const data = await daemon.call<SearchResultsData>('search', {
            file_id: info.file_id,
            query: msg.query,
            regex: msg.useRegex,
            max_results: 200,
          });
          webview.postMessage({ type: 'search_results', ...data });

        } else if (msg.type === 'count') {
          const data = await daemon.call<CountData>('count', {
            file_id: info.file_id,
            query: msg.query,
            regex: msg.useRegex,
          });
          webview.postMessage({ type: 'count_result', count: data.count, gen: msg.gen });
        }
      } catch (err: unknown) {
        webview.postMessage({ type: 'error', message: (err as Error).message });
      }
    }
  );

  disposable.onDidDispose(() => {
    messageSubscription.dispose();
    daemon.call('close', { file_id: info.file_id }).catch(() => {});
  });
}

function getNonce(): string {
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  return Array.from({ length: 32 }, () => chars[Math.floor(Math.random() * chars.length)]).join('');
}

function buildHtml(
  webview: vscode.Webview,
  extensionUri: vscode.Uri,
  info: OpenedData,
  sourcePath: string
): string {
  const nonce = getNonce();
  const cssUri = webview.asWebviewUri(vscode.Uri.joinPath(extensionUri, 'media', 'panel.css'));

  const jsPath = path.join(extensionUri.fsPath, 'media', 'panel.js');
  const jsContent = fs.readFileSync(jsPath, 'utf8');

  const config = JSON.stringify({
    totalLines: info.total_lines,
    fileSizeMb: (info.file_size / 1024 / 1024).toFixed(1),
    format: info.format,
    isJsonl: info.format === 'jsonl',
    isCsv: info.format === 'csv',
    csvDelimiter: path.extname(sourcePath).toLowerCase() === '.tsv' ? '\t' : ',',
  });

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'nonce-${nonce}'; style-src ${webview.cspSource};">
  <link rel="stylesheet" href="${cssUri}">
</head>
<body>
  <div id="toolbar">
    <input id="search" type="text" placeholder="Search in file..." />
    <button class="match-nav-btn" id="btn-match-prev" title="Previous match (Shift+Enter)" disabled>&#9650;</button>
    <button class="match-nav-btn" id="btn-match-next" title="Next match (Enter)" disabled>&#9660;</button>
    <span id="match-nav-info"></span>
    <button class="toggle-btn regex" id="btn-regex" title="Toggle regex (Ctrl+R)">.*</button>
    <button class="toggle-btn" id="btn-pretty" title="Toggle pretty-print JSON (JSONL only)">{}</button>
    <span class="sep">|</span>
    <input id="goto-input" type="text" placeholder="Line #" title="Go to line (Ctrl+G)" />
    <span id="match-count"></span>
    <span id="info"></span>
  </div>
  <div id="lines"><div id="status">Loading...</div></div>
  <div id="pagination">
    <button id="btn-prev" disabled>&#8592; Prev</button>
    <span id="page-info"></span>
    <button id="btn-next">Next &#8594;</button>
  </div>
  <div id="toast">Copied!</div>

  <script nonce="${nonce}">window.GLANCE_CONFIG = ${config};</script>
  <script nonce="${nonce}">${jsContent}</script>
</body>
</html>`;
}
