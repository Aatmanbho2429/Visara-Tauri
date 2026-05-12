import { ComponentFixture, TestBed } from '@angular/core/testing';

import { ActivityLog } from './activity-log';

describe('ActivityLog', () => {
  let component: ActivityLog;
  let fixture: ComponentFixture<ActivityLog>;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [ActivityLog],
    }).compileComponents();

    fixture = TestBed.createComponent(ActivityLog);
    component = fixture.componentInstance;
    await fixture.whenStable();
  });

  it('should create', () => {
    expect(component).toBeTruthy();
  });
});
