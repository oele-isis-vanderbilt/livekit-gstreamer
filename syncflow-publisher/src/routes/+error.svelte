<script lang="ts">
	import { page } from '$app/state';
</script>

<main class="container mx-auto flex flex-col w-full justify-start p-2 gap-4">
    <h1 class="text-2xl font-bold">Error</h1>
    <p class="text-red-500">An error occurred: {page.data.error.message}</p>
    <p class="text-gray-600">Please try again later or contact support.</p>
</main>