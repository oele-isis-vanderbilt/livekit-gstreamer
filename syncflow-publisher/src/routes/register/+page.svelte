<script lang="ts">
    import { goto } from "$app/navigation";
    import { error } from "@sveltejs/kit";
    import { invoke } from "@tauri-apps/api/core";
    import { Input, Label, Button } from "flowbite-svelte";
    import { onMount } from "svelte";

    let credentials = $state({
        syncflowProjectId: "",
        syncflowApiKey: "",
        syncflowServerUrl: "",
        syncflowApiSecret: "",
        deviceName: "",
        deviceGroup: "",
    });

    async function handleSubmit(event: Event) {
        event.preventDefault();
        try {
            const result = await invoke("register_to_syncflow", {
                credentials: {
                    syncflowProjectId: credentials.syncflowProjectId,
                    syncflowApiKey: credentials.syncflowApiKey,
                    syncflowServerUrl: credentials.syncflowServerUrl,
                    syncflowApiSecret: credentials.syncflowApiSecret,
                    deviceName:
                        credentials.deviceName === ""
                            ? null
                            : credentials.deviceName,
                    deviceGroup: credentials.deviceGroup,
                },
            });
            goto("/");
        } catch (err) {
            error(500, {
                message: `Registration failed: ${JSON.stringify(err)}`,
            });
        }
    }

    onMount(() => {
        // Check if the device is already registered
        invoke("get_registration")
            .then((registration) => {
                if (registration) {
                    goto("/");
                }
            })
            .catch((err) => {
                console.error("Failed to check registration:", err);
            });
    });
</script>

<main class="container mx-auto flex flex-col w-full justify-start p-2 gap-4">
    <h1 class="text-2xl font-bold">Welcome to SyncFlow!</h1>
    <h2 class="text-xl font-bold">Please register this device</h2>
    <form
        class="flex flex-col gap-4 w-full bg-white p-6 rounded shadow"
        onsubmit={handleSubmit}
    >
        <div>
            <Label
                for="syncflow-project-id"
                class="block text-sm font-medium text-gray-700 mb-1"
            >
                Syncflow Project ID
            </Label>
            <Input
                id="syncflow-project-id"
                type="text"
                required
                bind:value={credentials.syncflowProjectId}
                placeholder="Enter your Syncflow Project ID"
            />
        </div>
        <div>
            <Label
                for="syncflow-server-url"
                class="block text-sm font-medium text-gray-700 mb-1"
            >
                Syncflow Server URL
            </Label>
            <Input
                id="syncflow-server-url"
                type="text"
                bind:value={credentials.syncflowServerUrl}
                placeholder="Enter your Syncflow Server URL"
                required
            />
        </div>
        <div>
            <Label
                for="syncflow-api-key"
                class="block text-sm font-medium text-gray-700 mb-1"
            >
                Syncflow API Key
            </Label>
            <Input
                id="syncflow-api-key"
                type="text"
                bind:value={credentials.syncflowApiKey}
                placeholder="Enter your Syncflow API Key"
                required
            />
        </div>
        <div>
            <Label
                for="syncflow-api-secret"
                class="block text-sm font-medium text-gray-700 mb-1"
            >
                Syncflow API Secret
            </Label>
            <Input
                id="syncflow-api-secret"
                type="text"
                bind:value={credentials.syncflowApiSecret}
                placeholder="Enter your Syncflow API Secret"
                required
            />
        </div>
        <div>
            <Label
                for="device-name"
                class="block text-sm font-medium text-gray-700 mb-1"
            >
                Device Name
            </Label>
            <Input
                id="device-name"
                type="text"
                bind:value={credentials.deviceName}
                placeholder="Enter Device Name (Optional, defaults to hostname-ip-address)"
            />
        </div>
        <div>
            <Label
                for="device-group"
                class="block text-sm font-medium text-gray-700 mb-1"
            >
                Device Group
            </Label>
            <Input
                id="device-group"
                type="text"
                bind:value={credentials.deviceGroup}
                placeholder="Enter Device Group"
                required
            />
        </div>
        <Button type="submit">Register</Button>
    </form>
</main>
