import { ComponentFixture, TestBed } from '@angular/core/testing';

import { Master } from './master';

import { NEVER } from 'rxjs';
import { MessageService } from 'primeng/api';
import { provideRouter } from '@angular/router';
import { provideTranslateService } from '@ngx-translate/core';
import { ZoneWrapperService } from '../../core/zone-wrapper/zone-wrapper.service';

// No Tauri runtime under jsdom: every IPC call is inert.
const zoneWrapperStub = {
  invoke:              () => NEVER,
  invokeSilent:        () => NEVER,
  invokeFireAndForget: () => {},
  listen:              () => NEVER,
  toAssetUrl:          (p: string) => p,
};

describe('Master', () => {
  let component: Master;
  let fixture: ComponentFixture<Master>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [Master],
      providers: [provideRouter([]), MessageService, provideTranslateService(), { provide: ZoneWrapperService, useValue: zoneWrapperStub }],
    }).compileComponents();

    fixture = TestBed.createComponent(Master);
    component = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });
});
