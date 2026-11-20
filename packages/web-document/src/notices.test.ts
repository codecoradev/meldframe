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
