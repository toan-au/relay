<script lang="ts">
  import Navbar from './lib/Navbar.svelte';
  import Upload from './routes/Upload.svelte';
  import VideoPlayer from './routes/VideoPlayer.svelte';
  import Admin from './routes/Admin.svelte';

  let path = $state(window.location.pathname);

  window.addEventListener('popstate', () => {
    path = window.location.pathname;
  });

  type Route =
    | { page: 'upload' }
    | { page: 'video'; token: string }
    | { page: 'admin' };

  function parseRoute(path: string): Route {
    const videoMatch = path.match(/^\/video\/(.+)$/);
    if (videoMatch) return { page: 'video', token: videoMatch[1] };
    if (path === '/admin') return { page: 'admin' };
    return { page: 'upload' };
  }

  function navigate(to: string) {
    window.history.pushState({}, '', to);
    path = to;
  }

  let route = $derived(parseRoute(path));
</script>

<Navbar {navigate} />

{#if route.page === 'video'}
  <VideoPlayer token={route.token} {navigate} />
{:else if route.page === 'admin'}
  <Admin {navigate} />
{:else}
  <Upload {navigate} />
{/if}
