import { bestMatchBadgeKey, compareResults, isBestMatch, sortResults } from './result-order';
import { MatchKind } from '../../models/response/responseSearch';

// Minimal row with only the fields the ordering reads.
function row(path: string, matchKind: MatchKind, o: Partial<{ hashDistance: number | null; similarity: number; matchPoints: number; verified: boolean; mirrored: boolean; partial: boolean }> = {}) {
  return { path, matchKind, hashDistance: o.hashDistance ?? null, similarity: o.similarity ?? 50, matchPoints: o.matchPoints ?? 0,
           verified: o.verified ?? false, mirrored: o.mirrored ?? false, partial: o.partial ?? false, rank: 0 };
}

describe('result order', () => {
  // Same fixture and expectation as the Rust `compare_results_orders_tiers` test, so the two orders cannot drift.
  it('orders tiers exactly like the backend', () => {
    const rows = [
      row('z_none_verified', 'none', { similarity: 71, verified: true, matchPoints: 12 }),
      row('copy_30', 'copy', { similarity: 80, verified: true, matchPoints: 30 }),
      row('copy_90', 'copy', { similarity: 75, verified: true, matchPoints: 90 }),
      row('colour_d4', 'colourway', { hashDistance: 4, similarity: 99 }),
      row('colour_d1', 'colourway', { hashDistance: 1, similarity: 90 }),
      row('trans', 'exactTransformed', { hashDistance: 0, similarity: 90 }),
      row('exact_b', 'exact', { hashDistance: 2, similarity: 97 }),
      row('exact_a', 'exact', { hashDistance: 2, similarity: 99 }),
      row('exact_d0', 'exact', { hashDistance: 0, similarity: 60 }),
      row('none_sim95', 'none', { similarity: 95 }),
    ];
    sortResults(rows);
    expect(rows.map(r => r.path)).toEqual(
      ['exact_d0', 'exact_a', 'exact_b', 'trans', 'colour_d1', 'colour_d4', 'copy_90', 'copy_30', 'z_none_verified', 'none_sim95']);
    expect(rows.map(r => r.rank)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
  });

  // With every row 'none' this is the legacy order: verified first, then similarity, then path.
  it('keeps the legacy order when nothing is classified', () => {
    const rows = [row('b', 'none', { similarity: 80 }), row('a', 'none', { similarity: 80 }),
                  row('c', 'none', { similarity: 90 }), row('d', 'none', { similarity: 10, verified: true })];
    sortResults(rows);
    expect(rows.map(r => r.path)).toEqual(['d', 'c', 'a', 'b']);
  });

  // An unknown kind from an older backend must not outrank an exact row.
  it('treats an unknown matchKind as none', () => {
    const a = row('a', 'exact', { hashDistance: 5 });
    const b = { ...row('b', 'none'), matchKind: 'weird' as MatchKind };
    expect(compareResults(a, b)).toBeLessThan(0);
    expect(isBestMatch(b)).toBe(false);
  });

  // Equal in every key falls back to the path so the order is deterministic.
  it('breaks final ties by path', () => {
    expect(compareResults(row('b', 'none'), row('a', 'none'))).toBeGreaterThan(0);
    expect(compareResults(row('a', 'none'), row('a', 'none'))).toBe(0);
  });

  // The strip shows every engine verdict and nothing else.
  it('flags best matches', () => {
    expect((['exact', 'exactTransformed', 'colourway', 'copy'] as MatchKind[]).every(k => isBestMatch(row('x', k)))).toBe(true);
    expect(isBestMatch(row('x', 'none'))).toBe(false);
  });

  // One badge per kind, with mirrored and found-inside copies picking their own wording.
  it('maps each kind to its badge', () => {
    expect(bestMatchBadgeKey(row('x', 'exact'))).toBe('search.badgeExact');
    expect(bestMatchBadgeKey(row('x', 'exactTransformed'))).toBe('search.badgeRotated');
    expect(bestMatchBadgeKey(row('x', 'colourway'))).toBe('search.badgeColourway');
    expect(bestMatchBadgeKey(row('x', 'copy', { mirrored: true, partial: true }))).toBe('search.badgeRotated');
    expect(bestMatchBadgeKey(row('x', 'copy', { partial: true }))).toBe('search.badgeCopyInside');
    expect(bestMatchBadgeKey(row('x', 'copy'))).toBe('search.badgeCopy');
    expect(bestMatchBadgeKey(row('x', 'none'))).toBe('');
  });
});
