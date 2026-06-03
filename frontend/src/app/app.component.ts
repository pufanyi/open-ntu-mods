import { Component, inject } from "@angular/core";
import { RouterLink, RouterOutlet } from "@angular/router";
import { SessionService } from "./session.service";

@Component({
  selector: "app-root",
  standalone: true,
  imports: [RouterLink, RouterOutlet],
  template: `
    <div class="app-shell">
      <header class="topbar">
        <a routerLink="/" class="brand">Open NTU Mods</a>
        <nav class="nav">
          <a routerLink="/">Courses</a>
          <a routerLink="/admin">Admin</a>
          @if (session.user(); as user) {
            <a routerLink="/account">Account</a>
            <button type="button" (click)="logout()">Logout</button>
          } @else {
            <a routerLink="/login">Login</a>
          }
        </nav>
      </header>
      @if (session.user(); as user) {
        <div class="session-strip">
          {{ user.display_name || user.email }} · {{ user.role }}
        </div>
      }
      <main class="content">
        <router-outlet />
      </main>
    </div>
  `,
})
export class AppComponent {
  readonly session = inject(SessionService);

  constructor() {
    void this.session.load();
  }

  async logout(): Promise<void> {
    await this.session.logout();
  }
}
