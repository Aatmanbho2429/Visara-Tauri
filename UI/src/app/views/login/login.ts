import { Component, OnInit } from '@angular/core';
import { BaseComponent } from '../../core/base.component';
import { CommonModule } from '@angular/common';
import {
  ReactiveFormsModule,
  FormGroup,
  FormControl,
  Validators,
  AbstractControl,
  ValidationErrors
} from '@angular/forms';
import { Router } from '@angular/router';
import { TranslateModule } from '@ngx-translate/core';
import { MessageService } from 'primeng/api';
import { PrimengComponentsModule } from '../../shared/primeng-components-module';
import { AuthService } from '../../services/auth.service';
import { UserStateService } from '../../services/user-state.service';

function passwordMatchValidator(group: AbstractControl): ValidationErrors | null {
  const pw      = group.get('password')?.value;
  const confirm = group.get('confirm_password')?.value;
  if (confirm && pw !== confirm) {
    group.get('confirm_password')?.setErrors({ passwordMismatch: true });
    return { passwordMismatch: true };
  }
  if (confirm && pw === confirm) {
    const existing = group.get('confirm_password')?.errors;
    if (existing) {
      const { passwordMismatch, ...rest } = existing;
      group.get('confirm_password')?.setErrors(Object.keys(rest).length ? rest : null);
    }
  }
  return null;
}

function phoneValidator(control: AbstractControl): ValidationErrors | null {
  if (!control.value) return null;
  const valid = /^\+?[\d\s\-]{10,15}$/.test(control.value);
  return valid ? null : { phoneInvalid: true };
}

@Component({
  selector: 'app-login',
  imports: [CommonModule, ReactiveFormsModule, TranslateModule, PrimengComponentsModule],
  providers: [MessageService],
  templateUrl: './login.html',
  styleUrl: './login.scss'
})
export class Login extends BaseComponent implements OnInit {
  showRegister   = false;
  loginSuccess   = false;
  loginFirstName = '';
  loginError     = '';
  loginLoading   = false;

  registerSuccess = false;
  registerEmail   = '';
  registerError   = '';
  registerLoading = false;

  private redirectMessage = '';

  loginForm = new FormGroup({
    email:    new FormControl('', [Validators.required, Validators.email]),
    password: new FormControl('', [Validators.required, Validators.minLength(8)])
  });

  registerForm = new FormGroup(
    {
      first_name:       new FormControl('', [Validators.required, Validators.minLength(2)]),
      last_name:        new FormControl('', [Validators.required, Validators.minLength(2)]),
      email:            new FormControl('', [Validators.required, Validators.email]),
      phone_number:     new FormControl('', [phoneValidator]),
      company_name:     new FormControl(''),
      password:         new FormControl('', [Validators.required, Validators.minLength(8)]),
      confirm_password: new FormControl('', [Validators.required])
    },
    { validators: passwordMatchValidator }
  );

  get passwordStrength(): 'weak' | 'medium' | 'strong' {
    const pw = this.registerForm.get('password')?.value ?? '';
    if (pw.length < 8) return 'weak';
    const checks = [/[A-Z]/, /[a-z]/, /\d/, /[^a-zA-Z0-9]/].filter(r => r.test(pw)).length;
    if (checks >= 4) return 'strong';
    if (checks >= 2) return 'medium';
    return 'weak';
  }

  constructor(
    private router:         Router,
    private authService:    AuthService,
    private userState:      UserStateService,
    private messageService: MessageService
  ) {
    super();
    this.redirectMessage = sessionStorage.getItem('auth_redirect_msg') ?? '';
    sessionStorage.removeItem('auth_redirect_msg');
  }

  ngOnInit(): void {
    if (this.redirectMessage) {
      this.messageService.add({
        severity: 'warn',
        summary:  'Session Ended',
        detail:   this.redirectMessage,
        life:     6000
      });
    }
  }

  submitLogin(): void {
    if (this.loginForm.invalid) {
      this.loginForm.markAllAsTouched();
      return;
    }
    this.loginLoading = true;
    this.loginError   = '';
    const { email, password } = this.loginForm.value;
    this.handle(this.authService.login({ email: email!, password: password! }), res => {
      this.loginLoading = false;
      if (res.success && res.data) {
        this.loginSuccess   = true;
        this.loginFirstName = res.data.user.first_name;
        this.userState.set(res.data.user); // pre-populate so authGuard skips validate
        setTimeout(() => this.router.navigate(['/master']), 1500);
      } else {
        this.loginError = res.message;
      }
    });
  }

  submitRegister(): void {
    if (this.registerForm.invalid) {
      this.registerForm.markAllAsTouched();
      return;
    }
    this.registerLoading = true;
    this.registerError   = '';
    const v = this.registerForm.value;
    this.handle(this.authService.requestAccess({
      first_name:   v.first_name!,
      last_name:    v.last_name!,
      email:        v.email!,
      password:     v.password!,
      phone_number: v.phone_number ?? undefined,
      company_name: v.company_name ?? undefined
    }), res => {
      this.registerLoading = false;
      if (res.success) {
        this.registerSuccess = true;
        this.registerEmail   = v.email!;
      } else {
        this.registerError = res.message;
      }
    });
  }

  openRegister(): void {
    this.showRegister    = true;
    this.registerForm.reset();
    this.registerSuccess = false;
    this.registerError   = '';
  }

  closeRegister(): void {
    this.showRegister = false;
    this.loginForm.reset();
    this.loginSuccess = false;
    this.loginError   = '';
  }

  isInvalid(form: FormGroup, field: string): boolean {
    const ctrl = form.get(field);
    return !!(ctrl?.invalid && ctrl.touched);
  }

  hasError(form: FormGroup, field: string, error: string): boolean {
    return !!form.get(field)?.hasError(error);
  }
}
