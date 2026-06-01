import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { GlanceDaemon, OpenedData, LinesData, SearchResultsData, CountData } from './daemon';

type WebviewMessage =
  | { type: 'read'; offset: number; limit: number; pretty: boolean }
  | { type: 'search'; query: string; useRegex: boolean }
  | { type: 'count'; query: string; useRegex: boolean; gen: number };

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
      // retainContextWhenHidden removed — we use vscode.getState()/setState() instead.
      // This avoids high memory cost for large files while still preserving user state.
      localResourceRoots: [vscode.Uri.joinPath(context.extensionUri, 'media')],
    }
  );

  panel.webview.html = buildHtml(panel.webview, context.extensionUri, info);

  // Store subscription so it can be properly disposed.
  const messageSubscription = panel.webview.onDidReceiveMessage(
    async (msg: WebviewMessage) => {
      try {
        if (msg.type === 'read') {
          const data = await daemon.call<LinesData>('read', {
            file_id: info.file_id,
            offset: msg.offset,
            limit: msg.limit,
            pretty: msg.pretty,
          });
          panel.webview.postMessage({ type: 'lines', ...data });

        } else if (msg.type === 'search') {
          const data = await daemon.call<SearchResultsData>('search', {
            file_id: info.file_id,
            query: msg.query,
            regex: msg.useRegex,
            max_results: 200,
          });
          panel.webview.postMessage({ type: 'search_results', ...data });

        } else if (msg.type === 'count') {
          const data = await daemon.call<CountData>('count', {
            file_id: info.file_id,
            query: msg.query,
            regex: msg.useRegex,
          });
          panel.webview.postMessage({ type: 'count_result', count: data.count, gen: msg.gen });
        }
      } catch (err: unknown) {
        panel.webview.postMessage({ type: 'error', message: (err as Error).message });
      }
    }
  );

  panel.onDidDispose(() => {
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
  info: OpenedData
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
