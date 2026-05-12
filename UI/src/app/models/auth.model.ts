export interface User {
  id?: string;
  email: string;
  first_name: string;
  last_name: string;
  phone_number?: string;
  company_name?: string;
}

export interface LoginData {
  token: string;
  user: User;
}

export interface ValidateTokenData {
  user: User;
}
