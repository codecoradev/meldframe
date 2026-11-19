import { describe, expect, it } from 'vitest';
import { RELOAD_LIMIT, RELOAD_WINDOW_MS, describeSnapshotFailure, loadFile, reloadOnDivergence, shouldReload } from './file';

const answer = (status: number, body: unknown) => async () => new Response(typeof body === 'string' ? body : JSON.stringify(body), { status });

describe('loadFile', () => {
  it('reads the snapshot of a file', async () => {
    const f = await loadFile(answer(200, { seq: 4, nodes: [{ id: 'a' }] }) as typeof fetch, 'file_dev');
    expect(f).toEqual({ seq: 4, nodes: [{ id: 'a' }], variables: { collections: [], variables: [] }, attribution: {} });
  });
  it('keeps the attribution of the server (lastActor, lastSeq) apart from the nodes: the app layer refuses unknown fields', async () => {
    const f = await loadFile(
      answer(200, { seq: 9, nodes: [{ id: 'a', x: 1, lastActor: 'agent:dev', lastSeq: 7 }, { id: 'b', x: 2 }] }) as typeof fetch,
      'file_dev',
    );
    expect(f.nodes).toEqual([{ id: 'a', x: 1 }, { id: 'b', x: 2 }]);
    expect(f.attribution).toEqual({ a: { actor: 'agent:dev', seq: 7 } });
  });
  it('a file that does not exist yet is an empty file at sequence 0 (it comes to exist with its first op)', async () => {
    expect(await loadFile(answer(404, { error: { code: 'not_found' } }) as typeof fetch, 'file_dev')).toEqual({ seq: 0, nodes: [], variables: { collections: [], variables: [] }, attribution: {} });
  });
  it('takes the collections and variables of the snapshot along', async () => {
    const variables = { collections: [{ id: 't', name: 'theme', modes: [{ id: 'light', name: 'light' }] }], variables: [] };
    const f = await loadFile(answer(200, { seq: 2, nodes: [], variables }) as typeof fetch, 'file_dev');
    expect(f.variables).toEqual(variables);
  });
  it('asks for the file by its encoded name', async () => {
    let url = '';
    await loadFile((async (u: string) => { url = u; return new Response('{"seq":0,"nodes":[]}'); }) as unknown as typeof fetch, 'a b/c');
    expect(url).toBe('/v1/files/a%20b%2Fc');
  });
  it('any other status or a body of the wrong shape is an error that says what to do', async () => {
    await expect(loadFile(answer(401, 'no') as typeof fetch, 'f')).rejects.toThrow(/401.*token/s);
    await expect(loadFile(answer(403, 'no') as typeof fetch, 'f')).rejects.toThrow(/403/);
    await expect(loadFile(answer(500, 'boom') as typeof fetch, 'f')).rejects.toThrow(/500 \(boom\)/);
    for (const bad of [{ seq: -1, nodes: [] }, { seq: 1.5, nodes: [] }, { seq: 1 }, { seq: 1, nodes: {} }, 'not json', null]) {
      await expect(loadFile(answer(200, bad) as typeof fetch, 'f'), JSON.stringify(bad)).rejects.toThrow(/snapshot/);
    }
  });
  it('a network failure is an error too', async () => {
    await expect(loadFile((async () => { throw new TypeError('failed to fetch'); }) as unknown as typeof fetch, 'f')).rejects.toThrow(/failed to fetch/);
  });
});

describe('describeSnapshotFailure', () => {
  it('names the status and cuts a long body', () => {
    expect(describeSnapshotFailure(502, 'x'.repeat(500))).toHaveLength(`Could not load the file: the server answered 502 (${'x'.repeat(200)}).`.length);
  });
});

describe('shouldReload', () => {
  const store = () => { const m = new Map<string, string>(); return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v) }; };
  it('allows a few automatic reloads within a minute and then stops', () => {
    const s = store();
    for (let i = 0; i < RELOAD_LIMIT; i++) expect(shouldReload(s, 1000 + i)).toBe(true);
    expect(shouldReload(s, 2000)).toBe(false);
  });
  it('counts again after the window', () => {
    const s = store();
    for (let i = 0; i < RELOAD_LIMIT; i++) shouldReload(s, 1000);
    expect(shouldReload(s, 1000 + RELOAD_WINDOW_MS + 1)).toBe(true);
  });
  it('does not reload by itself when there is no storage to bound the loop', () => {
    expect(shouldReload({ getItem: () => { throw new Error('blocked'); }, setItem: () => {} }, 1)).toBe(false);
  });
});

describe('reloadOnDivergence', () => {
  const store = () => { const m = new Map<string, string>(); return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v) }; };
  it('asks for a reload when the app says that the file and the events no longer fit, and for nothing else', () => {
    expect(reloadOnDivergence([{ type: 'diverged' }], () => store(), 1)).toBe(true);
    expect(reloadOnDivergence([{ type: 'opsDropped' }, { type: 'connectionChanged' }], () => store(), 1)).toBe(false);
    expect(reloadOnDivergence([], () => store(), 1)).toBe(false);
  });
  it('stops after the limit, so a file that diverges again does not loop', () => {
    const s = store();
    const results = Array.from({ length: RELOAD_LIMIT + 1 }, (_, i) => reloadOnDivergence([{ type: 'diverged' }], () => s, 100 + i));
    expect(results).toEqual([...Array(RELOAD_LIMIT).fill(true), false]);
  });
  it('does not touch the storage unless the app diverged, and survives a storage that cannot even be reached', () => {
    let reached = 0;
    const blocked = () => { reached++; throw new DOMException('blocked', 'SecurityError'); };
    expect(reloadOnDivergence([{ type: 'historyChanged' }], blocked, 1)).toBe(false);
    expect(reached).toBe(0);
    expect(() => reloadOnDivergence([{ type: 'diverged' }], blocked, 1)).not.toThrow();
    expect(reloadOnDivergence([{ type: 'diverged' }], blocked, 1)).toBe(false);
  });
});
