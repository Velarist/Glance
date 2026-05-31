import * as vscode from 'vscode';
import * as path from 'path';
import { GlanceDaemon } from './daemon';
import { openFilePanel } from './panel';

let daemon: GlanceDaemon | undefined;

export function activate(context: vscode.ExtensionContext) {
  const daemonPath = path.join(context.extensionPath, 'bin', 'glance');
  daemon = new GlanceDaemon(daemonPath);

  context.subscriptions.push(
    vscode.commands.registerCommand('glance.openFile', async (uri?: vscode.Uri) => {
      if (!uri) {
        const picked = await vscode.window.showOpenDialog({
          filters: { 'Large Files': ['jsonl', 'ndjson', 'csv', 'tsv', 'log'] },
        });
        if (!picked?.[0]) { return; }
        uri = picked[0];
      }
      await openFilePanel(context, daemon!, uri);
    })
  );
}

export function deactivate() {
  daemon?.dispose();
}
