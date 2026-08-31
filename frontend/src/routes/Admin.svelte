<script lang="ts">
  import { onMount } from 'svelte';

  let { navigate }: { navigate: (to: string) => void } = $props();

  type Video = {
    share_token: string;
    title: string;
    status: string;
    view_count: number;
    created_at: string;
  };

  type Stats = {
    total_videos: number;
    total_views: number;
    by_status: Record<string, number>;
  };

  let checkingAuth = $state(true);
  let authed = $state(false);
  let password = $state('');
  let loginError = $state('');

  let videos = $state<Video[]>([]);
  let stats = $state<Stats | null>(null);
  let error = $state('');

  let editingToken = $state<string | null>(null);
  let editingTitle = $state('');

  async function loadData() {
    const [videosRes, statsRes] = await Promise.all([
      fetch('/api/admin/videos'),
      fetch('/api/admin/stats'),
    ]);
    if (videosRes.ok) videos = await videosRes.json();
    if (statsRes.ok) stats = await statsRes.json();
  }

  async function checkAuth() {
    const res = await fetch('/api/admin/stats');
    authed = res.ok;
    checkingAuth = false;
    if (authed) await loadData();
  }

  async function login() {
    loginError = '';
    const res = await fetch('/api/admin/login', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ password }),
    });
    if (res.ok) {
      authed = true;
      password = '';
      await loadData();
    } else {
      loginError = 'Wrong password';
    }
  }

  async function logout() {
    await fetch('/api/admin/logout', { method: 'POST' });
    authed = false;
    videos = [];
    stats = null;
  }

  async function deleteVideo(token: string) {
    if (!confirm('Delete this video? This cannot be undone.')) return;
    error = '';
    const res = await fetch(`/api/admin/videos/${token}`, { method: 'DELETE' });
    if (res.ok) {
      await loadData();
    } else {
      error = 'Failed to delete video';
    }
  }

  function startEdit(video: Video) {
    editingToken = video.share_token;
    editingTitle = video.title;
  }

  function cancelEdit() {
    editingToken = null;
  }

  async function saveEdit() {
    if (!editingToken || !editingTitle.trim()) return;
    error = '';
    const res = await fetch(`/api/admin/videos/${editingToken}`, {
      method: 'PATCH',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ title: editingTitle.trim() }),
    });
    if (res.ok) {
      const video = videos.find((v) => v.share_token === editingToken);
      if (video) video.title = editingTitle.trim();
      editingToken = null;
    } else {
      error = 'Failed to update title';
    }
  }

  onMount(() => {
    checkAuth();
  });
</script>

<div class="page">
  {#if checkingAuth}
    <p class="loading">Loading...</p>
  {:else if !authed}
    <div class="card login-card">
      <h1>Admin login</h1>
      <input
        type="password"
        placeholder="Password"
        bind:value={password}
        onkeydown={(e) => e.key === 'Enter' && login()}
      />
      <button class="primary" onclick={login}>Log in</button>
      {#if loginError}
        <p class="error">{loginError}</p>
      {/if}
    </div>
  {:else}
    <div class="dashboard">
      <div class="header">
        <h1>Admin dashboard</h1>
        <button class="ghost" onclick={logout}>Log out</button>
      </div>

      {#if stats}
        <div class="stats-row">
          <div class="stat-card">
            <span class="stat-value">{stats.total_videos}</span>
            <span class="stat-label">Videos</span>
          </div>
          <div class="stat-card">
            <span class="stat-value">{stats.total_views}</span>
            <span class="stat-label">Total views</span>
          </div>
          {#each Object.entries(stats.by_status) as [status, count] (status)}
            <div class="stat-card">
              <span class="stat-value">{count}</span>
              <span class="stat-label">{status}</span>
            </div>
          {/each}
        </div>
      {/if}

      {#if error}
        <p class="error">{error}</p>
      {/if}

      <div class="table-wrapper">
        <table>
          <thead>
            <tr>
              <th>Title</th>
              <th>Status</th>
              <th>Views</th>
              <th>Uploaded</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each videos as video (video.share_token)}
              <tr>
                <td>
                  {#if editingToken === video.share_token}
                    <input
                      class="edit-input"
                      bind:value={editingTitle}
                      onkeydown={(e) => e.key === 'Enter' && saveEdit()}
                    />
                  {:else}
                    {video.title}
                  {/if}
                </td>
                <td><span class="status-badge">{video.status}</span></td>
                <td>{video.view_count}</td>
                <td>{new Date(video.created_at).toLocaleDateString()}</td>
                <td class="actions">
                  {#if editingToken === video.share_token}
                    <button class="ghost small" onclick={saveEdit}>Save</button>
                    <button class="ghost small" onclick={cancelEdit}>Cancel</button>
                  {:else}
                    <button class="ghost small" onclick={() => navigate(`/video/${video.share_token}`)}>
                      View
                    </button>
                    <button class="ghost small" onclick={() => startEdit(video)}>Edit</button>
                    <button class="ghost small danger" onclick={() => deleteVideo(video.share_token)}>
                      Delete
                    </button>
                  {/if}
                </td>
              </tr>
            {:else}
              <tr>
                <td colspan="5" class="empty">No videos yet.</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  {/if}
</div>

<style>
  .page {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 2rem;
    background: var(--bg-subtle);
  }

  .loading {
    color: var(--text-muted);
  }

  .card {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 16px;
    padding: 2.5rem;
    width: 100%;
    max-width: 400px;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .login-card {
    margin: auto;
    text-align: center;
  }

  h1 {
    font-size: 1.5rem;
    font-weight: 700;
    color: var(--text);
  }

  input {
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.75rem 1rem;
    font-size: 1rem;
    color: var(--text);
    background: var(--bg-subtle);
    font-family: inherit;
  }

  input:focus {
    outline: none;
    border-color: var(--blue);
  }

  .error {
    color: #f87171;
    font-size: 0.9rem;
  }

  .dashboard {
    width: 100%;
    max-width: 1100px;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .stats-row {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .stat-card {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 1rem 1.5rem;
    display: flex;
    flex-direction: column;
    min-width: 100px;
  }

  .stat-value {
    font-size: 1.5rem;
    font-weight: 700;
    color: var(--text);
  }

  .stat-label {
    font-size: 0.8rem;
    color: var(--text-muted);
    text-transform: capitalize;
  }

  .table-wrapper {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 12px;
    overflow-x: auto;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  th {
    text-align: left;
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
    padding: 0.9rem 1.25rem;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }

  td {
    padding: 0.9rem 1.25rem;
    border-bottom: 1px solid var(--border);
    color: var(--text);
    white-space: nowrap;
  }

  tr:last-child td {
    border-bottom: none;
  }

  .empty {
    text-align: center;
    color: var(--text-muted);
    white-space: normal;
  }

  .status-badge {
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    background: var(--blue-light);
    color: var(--blue);
    padding: 0.2rem 0.7rem;
    border-radius: 9999px;
  }

  .edit-input {
    padding: 0.4rem 0.6rem;
    font-size: 0.95rem;
  }

  .actions {
    display: flex;
    gap: 0.5rem;
  }

  button.small {
    padding: 0.35rem 0.9rem;
    font-size: 0.85rem;
  }

  button.danger:hover:not(:disabled) {
    border-color: #f87171;
    color: #f87171;
  }
</style>
