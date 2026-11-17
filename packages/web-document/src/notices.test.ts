import type { Event } from '@meldframe/protocol';
import { describe, expect, it } from 'vitest';
import { noticeFor, noticeFromEvents } from './notices';

describe('noticeFor', () => {
  it('says what a refusal means in words', () => {
    expect(noticeFor({ type: 'refused', code: 'forbidden' } as Event)).toMatch(/not allowed/i);
    expect(noticeFor({ type: 'opRejected', key: 'k', code: 'conflict', retryable: false } as Event)).toMatch(/changed/i);
    expect(noticeFor({ type: 'refused', code: 'outbox_full' } as Event)).toMatch(/too many/i);
  });
  it('names an unknown code instead of hiding it', () => {
    expect(noticeFor({ type: 'refused', code: 'weird_thing' } as Event)).toContain('weird_thing');
  });
  it('is nothing for an event that is not a refusal, and for a rejection that will be sent again', () => {
    expect(noticeFor({ type: 'toolChanged', tool: 'select' } as Event)).toBeNull();
    expect(noticeFor({ type: 'opRejected', key: 'k', code: 'rate_limited', retryable: true } as Event)).toBeNull();
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
