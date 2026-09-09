import { ComponentFixture, TestBed } from '@angular/core/testing';

import { Search } from './search';

describe('Search', () => {
  let component: Search;
  let fixture: ComponentFixture<Search>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [Search],
    }).compileComponents();

    fixture = TestBed.createComponent(Search);
    component = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });

  // SEARCH-LATENCY-PLAN.md Phase 4 acceptance: a `search_partial` update
  // must not lose the client-only fields attached to an already-rendered
  // card (thumbnail, image-error state, the "found inside" box) — only the
  // backend fields it actually carries should change.
  it('merges a partial update by path without losing view-model fields', () => {
    const base: any = {
      path: 'a.png', name: 'a.png', similarity: 80, patternMatch: 80, colorMatch: 50,
      folder: 'f', verified: false, verification: 'unchecked', partial: false,
      matchRegion: { x: 0, y: 0, w: 1, h: 1 }, matchPoints: 0, mirrored: false, rank: 1,
      thumbnailUrl: 'asset://a', imgError: false, matchBox: { left: '1px', top: '1px', width: '2px', height: '2px' },
    };
    component.state.results = [base];

    (component as any).mergeResults([{ ...base, verified: true, verification: 'verified', matchPoints: 42 }]);

    expect(component.state.results.length).toBe(1);
    const merged = component.state.results[0];
    expect(merged.verified).toBe(true);
    expect(merged.verification).toBe('verified');
    expect(merged.matchPoints).toBe(42);
    // View-model fields — not part of the backend payload — must survive.
    expect(merged.thumbnailUrl).toBe('asset://a');
    expect(merged.matchBox).toEqual(base.matchBox);
  });

  it('appends a path not already in state.results as a new card', () => {
    component.state.results = [];
    (component as any).mergeResults([{
      path: 'b.png', name: 'b.png', similarity: 60, patternMatch: 60, colorMatch: 40,
      folder: 'f', verified: false, verification: 'unchecked', partial: false,
      matchRegion: { x: 0, y: 0, w: 1, h: 1 }, matchPoints: 0, mirrored: false, rank: 0,
    }]);
    expect(component.state.results.length).toBe(1);
    expect(component.state.results[0].path).toBe('b.png');
  });
});
