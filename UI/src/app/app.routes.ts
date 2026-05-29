import { Routes } from '@angular/router';
import { Login } from './views/login/login';
import { Master } from './views/master/master';
import { Search } from './views/search/search';
import { Profile } from './views/profile/profile';
import { Library } from './views/library/library';
import { authGuard } from './guards/auth.guard';
import { loginGuard } from './guards/login.guard';

export const routes: Routes = [
    { path: '', component: Login, canActivate: [loginGuard] },
    { path: 'master', component: Master, canActivate: [authGuard], children: [
        { path: '', redirectTo: 'search', pathMatch: 'full' },
        { path: 'search', component: Search },
        { path: 'library', component: Library },
        { path: 'profile', component: Profile },
    ]},
];
