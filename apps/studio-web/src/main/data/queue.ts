// A queue that runs one task at a time, in order, with cancellation (G21: a load can be cancelled). A task that has not
// started is taken out of the queue; one that has started is let finish (the reader's worker cannot be interrupted
// mid-read, and a data ticket of the service must be read to its end to keep the connection in step) and its result is
// dropped. Either way the caller's promise rejects with DataFailure('cancelled') at once.
import type { DataError, DataErrorCode } from '../../shared/data.js';

/** A failure with a data error code (or a service error code), carried to the page unchanged. */
export class DataFailure extends Error {
  readonly code: number | DataErrorCode;
  readonly data: unknown;
  constructor(code: number | DataErrorCode, message: string, data?: unknown) {
    super(message);
    this.name = 'DataFailure';
    this.code = code;
    this.data = data;
  }
  toDataError(): DataError {
    return this.data === undefined ? { code: this.code, message: this.message } : { code: this.code, message: this.message, data: this.data };
  }
}

export interface Job<T> {
  readonly promise: Promise<T>;
  /** Cancels the job: its promise rejects with `cancelled` (nothing happens once it has settled). */
  cancel(reason?: DataFailure): void;
}

interface Entry {
  readonly task: () => Promise<unknown>;
  readonly resolve: (v: unknown) => void;
  readonly reject: (e: unknown) => void;
  settled: boolean;
  started: boolean;
}

export class SerialQueue {
  private readonly entries: Entry[] = [];
  private running = false;

  /** Tasks waiting (not started). */
  get waiting(): number {
    return this.entries.length;
  }

  push<T>(task: () => Promise<T>): Job<T> {
    let entry!: Entry;
    const promise = new Promise<T>((resolve, reject) => {
      entry = { task, resolve: resolve as (v: unknown) => void, reject, settled: false, started: false };
    });
    this.entries.push(entry);
    this.pump();
    return {
      promise,
      cancel: (reason = new DataFailure('cancelled', 'cancelled')) => {
        if (entry.settled) return;
        entry.settled = true;
        if (!entry.started) {
          const i = this.entries.indexOf(entry);
          if (i >= 0) this.entries.splice(i, 1);
        }
        entry.reject(reason);
      },
    };
  }

  private pump(): void {
    if (this.running) return;
    const e = this.entries.shift();
    if (!e) return;
    this.running = true;
    e.started = true;
    void Promise.resolve()
      .then(() => e.task())
      .then(
        (v) => {
          if (!e.settled) {
            e.settled = true;
            e.resolve(v);
          }
        },
        (err: unknown) => {
          if (!e.settled) {
            e.settled = true;
            e.reject(err);
          }
        },
      )
      .finally(() => {
        this.running = false;
        this.pump();
      });
  }
}
