import { Routes } from '@angular/router';
import { Login } from './views/login/login';
import { Master } from './views/master/master';
import { Search } from './views/search/search';
import { Profile } from './views/profile/profile';
import { Library } from './views/library/library';
import { Browse } from './views/browse/browse';
import { authGuard } from './guards/auth.guard';
import { loginGuard } from './guards/login.guard';
import { subscriptionGuard } from './guards/subscription.guard';

export const routes: Routes = [
    { path: '', component: Login, canActivate: [loginGuard] },
    { path: 'master', component: Master, canActivate: [authGuard], children: [
        { path: '', redirectTo: 'search', pathMatch: 'full' },
        // Feature pages require an active subscription (trial/active).
        { path: 'search',         component: Search,  canActivate: [subscriptionGuard] },
        { path: 'library',        component: Library, canActivate: [subscriptionGuard] },
        { path: 'browse',         component: Browse,  canActivate: [subscriptionGuard] },
        // Profile stays open so expired users can renew.
        { path: 'profile',        component: Profile },
    ]},
];
