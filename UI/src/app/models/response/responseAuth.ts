// Auth response payloads — mirrors UI/src-tauri/src/models/response/response_auth.rs.
export interface User {
  id: string;
  email: string;
  firstName: string;
  lastName: string;
  phoneNumber?: string;
  companyName?: string;
  subscriptionStatus: string;
  subscriptionEnd: string | null;
  daysRemaining: number | null;
}

export interface responseLogin {
  token: string;
  user:  User;
}

export interface responseValidateToken {
  user: User;
}

// `action` mirrors what the periodic-revalidate tick decided to do — see
// services::auth::periodic_revalidate for the meaning of each value.
export interface responsePeriodicRevalidate {
  action: 'none' | 'ok' | 'ok-offline' | 'logout';
  user?:  User;
}
