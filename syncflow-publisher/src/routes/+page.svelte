<script lang="ts">
    import { goto } from "$app/navigation";
    import { error } from "@sveltejs/kit";
  import { invoke } from "@tauri-apps/api/core";
    import { Button } from "flowbite-svelte";
  import { onMount } from "svelte";

  let registrationDetails = $state(null);

  onMount(async () => {
    try {
      const registration = await invoke("get_registration");
      if (registration) {
        registrationDetails = registration;
        console.log("Registration details fetched:", registrationDetails);
      } else {
        error(404, { message: "Registration details not found." });
      }
    } catch (err) {
      goto("/register");
    }
  });

  function deregister() {
    invoke("delete_registration")
      .then(() => {
        goto("/register");
      })
      .catch((err) => {
        error(500, { message: `Deregistration failed: ${JSON.stringify(err)}` });
      });
  }


</script>

<main class="container mx-auto flex flex-col w-full justify-start p-2 gap-4">
  <div class="flex justify-between items-center">
    <h1 class="text-xl font-bold flex-1">Welcome to SyncFlow Publisher!,  {registrationDetails?.deviceName}</h1>
    <Button
      color="red"
      onclick={deregister}
      >Delete Registration</Button>
  </div>
  {#if registrationDetails}
    <div class="bg-white rounded-lg shadow-md p-6 mt-4">
      <h2 class="text-lg font-semibold mb-4">Device & Project Details</h2>
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div>
          <div class="font-medium text-gray-700">Device ID:</div>
          <div class="text-gray-900">{registrationDetails.deviceId}</div>
        </div>
        <div>
          <div class="font-medium text-gray-700">Device Name:</div>
          <div class="text-gray-900">{registrationDetails.deviceName}</div>
        </div>
        <div>
          <div class="font-medium text-gray-700">Device Group:</div>
          <div class="text-gray-900">{registrationDetails.deviceGroup}</div>
        </div>
        <div>
          <div class="font-medium text-gray-700">Project Name:</div>
          <div class="text-gray-900">{registrationDetails.projectName}</div>
        </div>
        <div>
          <div class="font-medium text-gray-700">Project ID:</div>
          <div class="text-gray-900">{registrationDetails.projectId}</div>
        </div>
        <div>
          <div class="font-medium text-gray-700">Project Comments:</div>
          <div class="text-gray-900">{registrationDetails.projectComments}</div>
        </div>
        <div>
          <div class="font-medium text-gray-700">LiveKit Server URL:</div>
          <div class="text-gray-900">{registrationDetails.lkServerUrl}</div>
        </div>
        <div>
          <div class="font-medium text-gray-700">S3 Bucket Name:</div>
          <div class="text-gray-900">{registrationDetails.s3BucketName}</div>
        </div>
        <div>
          <div class="font-medium text-gray-700">S3 Endpoint:</div>
          <div class="text-gray-900">{registrationDetails.s3Endpoint}</div>
        </div>
      </div>
    </div>
  {/if}
</main>
