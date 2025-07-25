<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import {
    MultiSelect
  } from 'flowbite-svelte';
  import type {
    SelectOptionType,
  } from 'flowbite-svelte';


  let devices = $state([]);

  onMount(async () => {
    try {
      devices = await invoke("get_devices");
    } catch (error) {
      console.error("Failed to fetch devices:", error);
    }
  });

  let videoDevices = $derived.by(() => {
    return devices.filter(device => device.device_class === "Video/Source")
  });

  let audioDevices = $derived.by(() => {
    return devices.filter(device => device.device_class === "Audio/Source")
  });

  let monitors = $derived.by(() => {
    return devices.filter(device => device.device_class === "Screen/Source")  
  });

  let videoSelectItems: SelectOptionType<string> = $derived.by(() => {
    let items: { name: string; value: string }[] = [];
    videoDevices.forEach((device, idx) => {
      if (device.capabilities && Array.isArray(device.capabilities)) {
        device.capabilities.forEach((capability, capIdx) => {
          items.push({
            name: `${device.display_name || `Video Device ${idx + 1}`} - ${capability}`,
            value: `${device.device_path}::${capability}`
          });
        });
      } else {
        items.push({
          name: device.display_name || `Video Device ${idx + 1}`,
          value: device.device_path
        });
      }
    });
    return items;
  });
  let selectedVideoDevicePaths: string[] = $state([]);

  let selectedVideoDevices = $derived.by(() => {
    return videoDevices.filter(device => selectedVideoDevicePaths.includes(device.device_path));
  });
</script>

<main class="container mx-auto flex flex-col w-full justify-start p-2 gap-4">
  <h1 class="text-2xl font-bold">Welcome to SyncFlow Publisher</h1>
  <h2 class="text-xl font-bold">Select Video Devices</h2>
  <MultiSelect items={videoSelectItems} bind:value={selectedVideoDevicePaths} placeholder="Select video devices" />

  <div class="flex flex-row items-center justify-between w-full h-full">
    <pre>{JSON.stringify(selectedVideoDevices, null, 2)}</pre>
  </div>
</main>
