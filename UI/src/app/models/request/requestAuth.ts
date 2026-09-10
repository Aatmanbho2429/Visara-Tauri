// Auth request payloads — mirrors UI/src-tauri/src/models/request/request_auth.rs.

export interface requestLogin {
  email: string;
  password: string;
}

// Shared by every "email a one-time code" call — sendOtp, forgotPasswordSendOtp.
export interface requestEmail {
  email: string;
}

export interface requestVerifyOtp {
  email: string;
  otpCode: string;
}

export interface requestChangePassword {
  oldPassword: string;
  newPassword: string;
}

export interface requestRequestAccess {
  firstName: string;
  lastName: string;
  email: string;
  password: string;
  phoneNumber?: string;
  companyName?: string;
  otpCode: string;
}
