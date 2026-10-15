import { TestBed } from '@angular/core/testing';
import { App } from './app';

import { NEVER } from 'rxjs';
import { provideRouter } from '@angular/router';
import { provideTranslateService } from '@ngx-translate/core';
import { ZoneWrapperService } from './core/zone-wrapper/zone-wrapper.service';

// No Tauri runtime under jsdom: every IPC call is inert.
const zoneWrapperStub = {
  invoke:              () => NEVER,
  invokeSilent:        () => NEVER,
  invokeFireAndForget: () => {},
  listen:              () => NEVER,
  toAssetUrl:          (p: string) => p,
};

describe('App', () => {
  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [App],
      providers: [provideRouter([]), provideTranslateService(), { provide: ZoneWrapperService, useValue: zoneWrapperStub }],
    }).compileComponents();
  });

  it('should create the app', () => {
    const fixture = TestBed.createComponent(App);
    const app = fixture.componentInstance;
    expect(app).toBeTruthy();
  });

  it('should render the router outlet', async () => {
    const fixture = TestBed.createComponent(App);
    await fixture.whenStable();
    const compiled = fixture.nativeElement as HTMLElement;
    expect(compiled.querySelector('router-outlet')).toBeTruthy();
  });
});
