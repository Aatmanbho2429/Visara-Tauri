import { ComponentFixture, TestBed } from '@angular/core/testing';
import { NEVER, of } from 'rxjs';
import { provideTranslateService } from '@ngx-translate/core';
import { ZoneWrapperService } from '../../core/zone-wrapper/zone-wrapper.service';

import { Search } from './search';
import { SearchService } from '../../services/search/search.service';
import { SearchStateService } from '../../services/search/search-state.service';

// No Tauri runtime under jsdom: every IPC call is inert and the sidecar reports not-ready.
const zoneWrapperStub = {
  invoke:              () => NEVER,
  invokeSilent:        () => of({ ready: false, healthy: false, embedReady: false }),
  invokeFireAndForget: () => {},
  listen:              () => NEVER,
  toAssetUrl:          (p: string) => p,
};

describe('Search', () => {
  let component: Search;
  let fixture: ComponentFixture<Search>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [Search],
      providers: [provideTranslateService(), { provide: ZoneWrapperService, useValue: zoneWrapperStub }],
    }).compileComponents();

    fixture = TestBed.createComponent(Search);
    component = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });

  // A `search_partial` update
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

  // Stopping on "New search": a search still running behind the grid must be cancelled in Rust too.
  it('newSearch() cancels a running search', () => {
    const cancel = vi.spyOn(TestBed.inject(SearchService), 'cancel').mockImplementation(() => {});
    component.state.progress = { ...component.state.progress, active: true };
    component.newSearch();
    expect(cancel).toHaveBeenCalledTimes(1);
  });

  it('newSearch() does not cancel when no search is active', () => {
    const cancel = vi.spyOn(TestBed.inject(SearchService), 'cancel').mockImplementation(() => {});
    component.state.progress = { ...component.state.progress, active: false };
    component.newSearch();
    expect(cancel).not.toHaveBeenCalled();
  });

  it('stopSearch() cancels once and shows the stopping state', () => {
    const cancel = vi.spyOn(TestBed.inject(SearchService), 'cancel').mockImplementation(() => {});
    component.stopSearch();
    component.stopSearch();
    expect(cancel).toHaveBeenCalledTimes(1);
    expect(component.stopping).toBe(true);
  });
});

describe('Search deep-verify floor', () => {
  beforeEach(() => TestBed.resetTestingModule());

  it('defaults to 70%', () => {
    expect(TestBed.inject(SearchStateService).deepMinSim).toBe(0.7);
  });

  it('is not remembered across app restarts', () => {
    TestBed.inject(SearchStateService).setDeepMinSim(0.4);
    TestBed.resetTestingModule();
    expect(TestBed.inject(SearchStateService).deepMinSim).toBe(0.7);
  });

  it('goes back to 70% on reset()', () => {
    const state = TestBed.inject(SearchStateService);
    state.setDeepMinSim(0.5);
    state.reset();
    expect(state.deepMinSim).toBe(0.7);
  });
});

describe('Search Full DB option', () => {
  beforeEach(() => TestBed.resetTestingModule());

  it('defaults to off', () => {
    expect(TestBed.inject(SearchStateService).fullDb).toBe(false);
  });

  it('is cleared by reset()', () => {
    const state = TestBed.inject(SearchStateService);
    state.setFullDb(true);
    state.reset();
    expect(state.fullDb).toBe(false);
  });

  it('is cleared by picking a percentage', () => {
    const state = TestBed.inject(SearchStateService);
    state.setFullDb(true);
    state.setDeepMinSim(0.6);
    expect(state.fullDb).toBe(false);
    expect(state.deepMinSim).toBe(0.6);
  });
});
