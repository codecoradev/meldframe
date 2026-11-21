import { readFileSync } from 'node:fs';
import type { Event } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { noticeFor, noticeFromEvents } from './notices';

describe('noticeFor', () => {
  it('says what a refusal means in words', () => {
    expect(noticeFor({ type: 'refused', code: 'forbidden' } as Event)).toMatch(/not allowed/i);
    expect(noticeFor({ type: 'opRejected', key: 'k', code: 'conflict', retryable: false } as Event)).toMatch(/changed/i);
    expect(noticeFor({ type: 'refused', code: 'outbox_full' } as Event)).toMatch(/too many/i);
  });
  it('says the storage of the workspace is full, that the files are unchanged, and that edits that add data wait', () => {
    const text = noticeFor({ type: 'opRejected', key: 'k', code: 'quota_storage', retryable: false } as Event)!;
    expect(text).toMatch(/storage is full/i);
    expect(text).toMatch(/files are unchanged/i);
    expect(text).not.toContain('quota_storage');
  });
  it('names an unknown code instead of hiding it', () => {
    expect(noticeFor({ type: 'refused', code: 'weird_thing' } as Event)).toContain('weird_thing');
  });
  it('is nothing for an event that is not a refusal, and for a rejection that will be sent again', () => {
    expect(noticeFor({ type: 'toolChanged', tool: 'select' } as Event)).toBeNull();
    expect(noticeFor({ type: 'opRejected', key: 'k', code: 'rate_limited', retryable: true } as Event)).toBeNull();
  });
});

describe('noticeFor refusals of the panel and of the document checks', () => {
  const refused = (code: string) => ({ type: 'refused', code }) as Event;
  it('words the refusals of the Variables panel without blaming the server', () => {
    for (const code of ['invalid_value', 'not_bindable', 'E_INVALID_COLOR', 'E_NOT_FINITE', 'E_OUT_OF_RANGE', 'E_TYPE_MISMATCH', 'E_ALIAS_CYCLE', 'E_VARIABLE_NOT_FOUND']) {
      const text = noticeFor(refused(code))!;
      expect(text, code).not.toMatch(/server/i);
      expect(text, code).not.toContain(code);
    }
  });
  it('says a refusal made here was not made, and keeps "the server" for what the server refused', () => {
    expect(noticeFor(refused('E_SOMETHING_NEW'))).toMatch(/was not made.*E_SOMETHING_NEW/);
    expect(noticeFor({ type: 'opRejected', key: 'k', code: 'E_SOMETHING_NEW', retryable: false } as Event)).toMatch(/server refused/i);
  });
});

describe('noticeFor approvals', () => {
  const rejected = (code: string) => ({ type: 'opRejected', key: 'k', code, retryable: false }) as Event;
  it('says a delete needs the owner\'s approval and was not done, without promising an answer screen', () => {
    const text = noticeFor(rejected('approval_required'))!;
    expect(text).toMatch(/needs the owner's approval/i);
    expect(text).toMatch(/not done/i);
    expect(text).not.toMatch(/not available yet/i);
  });
  it('has its own words for denied, expired and too many open approvals', () => {
    expect(noticeFor(rejected('approval_denied'))).toMatch(/owner said no/i);
    expect(noticeFor(rejected('approval_expired'))).toMatch(/ran out/i);
    expect(noticeFor(rejected('approval_limit'))).toMatch(/too many approvals/i);
    for (const code of ['approval_required', 'approval_denied', 'approval_expired', 'approval_limit']) {
      expect(noticeFor(rejected(code))).not.toContain(code);
    }
  });
});

describe('noticeFromEvents', () => {
  it('is the message of the last refusal among the events', () => {
    const events = [
      { type: 'refused', code: 'forbidden' },
      { type: 'toolChanged', tool: 'select' },
      { type: 'refused', code: 'conflict' },
    ] as Event[];
    expect(noticeFromEvents(events)).toMatch(/changed/i);
  });
  it('is null when no event is a refusal', () => {
    expect(noticeFromEvents([{ type: 'toolChanged', tool: 'select' } as Event])).toBeNull();
    expect(noticeFromEvents([])).toBeNull();
  });
});

describe('noticeFor refusals of the inspector', () => {
  const refused = (code: string) => ({ type: 'refused', code }) as Event;
  it('words what the core and the app layer refuse in the fields, naming no code', () => {
    for (const code of ['invalid', 'limit', 'min_stops', 'no_gradient', 'no_image', 'gradient_on_text', 'not_selected']) {
      const text = noticeFor(refused(code))!;
      expect(text, code).not.toContain(code);
      expect(text, code).not.toMatch(/server|was not made/i);
    }
  });
});

// The Variables manage view (D8): every code its actions can come back with has words of its own. The codes are read
// from the Rust sources and the table of docs/error-codes.md, so a new code of the core fails here until it is sorted.
describe('the codes of the Variables manage actions', () => {
  const root = new URL('../../../', import.meta.url);
  const read = (path: string) => readFileSync(new URL(path, root), 'utf8');
  const errors = read('crates/core/meldframe-variables/src/error.rs');
  const listed = errors.slice(errors.indexOf('VAR_ERROR_CODES'), errors.indexOf('];'));
  const common = errors.slice(errors.indexOf('A value rule that nodes share'), errors.indexOf('Common {'));
  const codes = [...listed.matchAll(/"(E_[A-Z_]+)"/g), ...common.matchAll(/`(E_[A-Z_]+)`/g)].map((m) => m[1]!);
  // What the six actions can come back with: from the app layer (the check of the group, a name that is refused) or
  // from the server (an op that lost a race with another actor's).
  const REACHABLE = ['E_INVALID_NAME', 'E_DUPLICATE_NAME', 'E_TOO_LONG', 'E_TOO_MANY_ENTRIES', 'E_COLLECTION_NOT_FOUND', 'E_MODE_NOT_FOUND', 'E_VARIABLE_NOT_FOUND', 'E_DEFAULT_MODE', 'E_VARIABLE_IN_USE', 'E_COLLECTION_NOT_EMPTY', 'E_INCOMPLETE_VALUES', 'E_INVALID_MODES', 'E_DUPLICATE_ID'];
  // The rest belong to values, bindings and aliases (the Variables panel words them) or to a caller that is not a replica;
  // the ids and the values of the manage actions are made by the app layer.
  const NOT_REACHABLE = ['E_TYPE_MISMATCH', 'E_ALIAS_CYCLE', 'E_ALIAS_TOO_DEEP', 'E_NO_VARIABLES', 'E_INVALID_ID', 'E_INVALID_COLOR', 'E_NOT_FINITE', 'E_OUT_OF_RANGE'];
  const refused = (code: string) => ({ type: 'refused', code }) as Event;
  const rejected = (code: string) => ({ type: 'opRejected', key: 'k', code, node: null, retryable: false }) as Event;

  it('sorts every variable code of the core as reachable from the actions or not', () => {
    expect(codes.length).toBeGreaterThan(15);
    for (const code of codes) expect([...REACHABLE, ...NOT_REACHABLE], code).toContain(code);
    for (const code of [...REACHABLE, ...NOT_REACHABLE]) expect(codes, code).toContain(code);
  });
  it('has words for each reachable code, as a refusal of the app and as a rejection by the server, with no code in them', () => {
    for (const code of [...REACHABLE, 'outbox_full']) {
      for (const e of [refused(code), rejected(code)]) {
        const text = noticeFor(e)!;
        expect(text, code).not.toContain(code);
        expect(text, code).not.toMatch(/^The (change was not made|server refused)/);
        expect(text.length, code).toBeGreaterThan(20);
      }
    }
  });
  it('lists each reachable code in the table of docs/error-codes.md', () => {
    const doc = read('docs/error-codes.md');
    for (const code of REACHABLE) expect(doc, code).toContain(`| \`${code}\` |`);
  });
  it('says what to do about a removal that was refused, in the words of the view (Detach)', () => {
    // The question has closed when the refusal arrives: the words say to ask again.
    expect(noticeFor(refused('E_VARIABLE_IN_USE'))).toMatch(/Remove it again and choose Detach/);
    expect(noticeFor(refused('E_COLLECTION_NOT_EMPTY'))).toMatch(/Remove it again and choose Detach/);
    expect(noticeFor(refused('outbox_full'))).toMatch(/nothing was changed|not changed/i);
  });
  it('keeps the words of the limits and of the id that the nodes share free of variables, collections and modes', () => {
    // The core uses these three for nodes, texts, shadows and gradient stops too (meldframe-check error.rs): the toast of the
    // editor must not talk about variables. The manage view words them for what it sent (manage.ts).
    for (const code of ['E_TOO_LONG', 'E_TOO_MANY_ENTRIES', 'E_DUPLICATE_ID']) {
      for (const e of [refused(code), rejected(code)]) expect(noticeFor(e), code).not.toMatch(/variable|collection|mode|name/i);
    }
  });
  it('words a default mode that cannot go, and a duplicate name, as the rule of the view', () => {
    expect(noticeFor(refused('E_DEFAULT_MODE'))).toMatch(/first mode/i);
    expect(noticeFor(refused('E_DUPLICATE_NAME'))).toMatch(/already/i);
    expect(noticeFor(refused('E_INVALID_NAME'))).toMatch(/lowercase/i);
  });
});
