import * as cp from 'child_process';
import * as readline from 'readline';
import * as vscode from 'vscode';

export const PROTOCOL_VERSION = 1;

export interface RpcResponse {
  jsonrpc: string;
  id: number | null;
  result?: unknown;
  error?: { code: number; message: string };
}

export interface OpenedData {
  file_id: number;
  total_lines: number;
  file_size: number;
  format: string;
}

export interface LinesData {
  lines: Array<{ number: number; content: string; fields?: string[] }>;
  total_lines: number;
  offset: number;
}

export interface SearchResultsData {
  results: Array<{ line_number: number; content: string; match_start: number; match_end: number }>;
  total_found: number;
  truncated: boolean;
}

export interface CountData {
  count: number;
}

export class GlanceDaemon {
  private proc: cp.ChildProcess;
  private rl: readline.Interface;
  private pending = new Map<number, (r: RpcResponse) => void>();
  private nextId = 1;
  private alive = true;

  constructor(daemonPath: string) {
    this.proc = cp.spawn(daemonPath, [], { stdio: ['pipe', 'pipe', 'inherit'] });

    this.rl = readline.createInterface({ input: this.proc.stdout! });
    this.rl.on('line', (line: string) => {
      const res: RpcResponse = JSON.parse(line);
      if (res.id !== null && res.id !== undefined) {
        const cb = this.pending.get(res.id as number);
        if (cb) {
          this.pending.delete(res.id as number);
          cb(res);
        }
      }
    });

    this.proc.on('error', (err: Error) => {
      vscode.window.showErrorMessage(`Glance daemon error: ${err.message}`);
      this.rejectAll(`daemon error: ${err.message}`);
    });

    this.proc.on('exit', (code: number | null) => {
      this.alive = false;
      this.rejectAll(`daemon exited with code ${code}`);
    });
  }

  private rejectAll(reason: string) {
    for (const [id, cb] of this.pending) {
      cb({ jsonrpc: '2.0', id, error: { code: -1, message: reason } });
    }
    this.pending.clear();
  }

  call<T>(method: string, params: unknown): Promise<T> {
    if (!this.alive) {
      return Promise.reject(new Error('daemon is not running'));
    }
    return new Promise((resolve, reject) => {
      const id = this.nextId++;
      this.pending.set(id, (res) => {
        if (res.error) {
          reject(new Error(res.error.message));
        } else {
          resolve(res.result as T);
        }
      });
      const msg = JSON.stringify({ jsonrpc: '2.0', id, version: PROTOCOL_VERSION, method, params }) + '\n';
      this.proc.stdin!.write(msg);
    });
  }

  dispose() {
    this.alive = false;
    this.proc.kill();
  }
}
