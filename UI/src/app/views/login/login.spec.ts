import { ComponentFixture, TestBed } from '@angular/core/testing';

import { Login } from './login';

import { NEVER } from 'rxjs';
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

describe('Login', () => {
  let component: Login;
  let fixture: ComponentFixture<Login>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [Login],
      providers: [provideRouter([]), provideTranslateService(), { provide: ZoneWrapperService, useValue: zoneWrapperStub }],
    }).compileComponents();

    fixture = TestBed.createComponent(Login);
    component = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });
});
