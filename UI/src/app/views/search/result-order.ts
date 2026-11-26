import { MatchKind, SearchResult } from '../../models/response/responseSearch';

// Tier of each match kind, best first; must match `services::search::match_tier` in Rust.
const TIER: Record<MatchKind, number> = { exact: 0, exactTransformed: 1, colourway: 2, copy: 3, none: 4 };

// The three tiers the pHash decides, ordered by hash distance.
const isHashTier = (k: MatchKind): boolean => k === 'exact' || k === 'exactTransformed' || k === 'colourway';

// Unknown strings from an older backend read as 'none' so they can never outrank a real match.
const kindOf = (r: Pick<SearchResult, 'matchKind'>): MatchKind => (r.matchKind && r.matchKind in TIER ? r.matchKind : 'none');

type Orderable = Pick<SearchResult, 'matchKind' | 'hashDistance' | 'similarity' | 'matchPoints' | 'verified' | 'path'>;

// Backend `compare_results` ported exactly: tier, then the within-tier key, then path ascending.
export function compareResults(a: Orderable, b: Orderable): number {
  const ka = kindOf(a);
  const kb = kindOf(b);
  if (TIER[ka] !== TIER[kb]) return TIER[ka] - TIER[kb];
  let within = 0;
  if (isHashTier(ka)) {
    within = (a.hashDistance ?? Infinity) === (b.hashDistance ?? Infinity)
      ? b.similarity - a.similarity
      : (a.hashDistance ?? Infinity) < (b.hashDistance ?? Infinity) ? -1 : 1;
  } else if (ka === 'copy') {
    within = b.matchPoints - a.matchPoints;
  } else {
    within = (Number(b.verified) - Number(a.verified)) || (b.similarity - a.similarity);
  }
  if (within) return within;
  return a.path < b.path ? -1 : a.path > b.path ? 1 : 0;
}

// Sorts in place with the backend order and numbers the rows 1..n by their final position.
export function sortResults<T extends Orderable & { rank: number }>(results: T[]): T[] {
  results.sort(compareResults);
  results.forEach((r, i) => { r.rank = i + 1; });
  return results;
}

// True for rows the engine decided (any kind but 'none'); these lead the page in the Best matches strip.
export const isBestMatch = (r: Pick<SearchResult, 'matchKind'>): boolean => kindOf(r) !== 'none';

// i18n key of the one badge a Best matches card shows.
export function bestMatchBadgeKey(r: Pick<SearchResult, 'matchKind' | 'mirrored' | 'partial'>): string {
  switch (kindOf(r)) {
    case 'exact': return 'search.badgeExact';
    case 'exactTransformed': return 'search.badgeRotated';
    case 'colourway': return 'search.badgeColourway';
    case 'copy':
      if (r.mirrored) return 'search.badgeRotated';
      return r.partial ? 'search.badgeCopyInside' : 'search.badgeCopy';
    default: return '';
  }
}
