<script lang="ts">
    import type { Mod } from "../../stores/modStore";
    import { Download, Trash2 } from "lucide-svelte";
	import {
		installationStatus,
		loadingStates2 as loadingStates,
	} from "../../stores/modStore";
	import { stripMarkdown, truncateText } from "../../utils/helpers";

    interface Props {
        mod: Mod,
        onmodclick?: (mod: Mod) => void,
        oninstallclick?: (mod: Mod) => void,
        onuninstallclick?: (mod: Mod) => void
    }

    let { mod, oninstallclick, onuninstallclick, onmodclick }: Props = $props();

	function installMod(e: Event) {
		e.stopPropagation();
		if (oninstallclick) oninstallclick(mod)
	}

	function uninstallMod(e: Event) {
		e.stopPropagation();
		if (onuninstallclick) onuninstallclick(mod)
	}

	function openModView() {
		if (onmodclick) onmodclick(mod)
	}

	/* Later, for CSS
		.tag {
			display: flex;
			align-items: center;
			position: relative;
			gap: 0.2rem;
			padding: 0.15rem 0.3rem;
			background: rgba(0, 0, 0, 0.7);
			border-radius: 4px;
			font-size: 0.9rem;
			color: #f4eee0;
		}
	*/

</script>

<div
    class="mod-card"
    onclick={openModView}
    onkeydown={(e) => e.key === "Enter" && openModView()}
    role="button"
    tabindex="0"
    style="--orig-color1: {mod.colors.color1}; --orig-color2: {mod.colors.color2};"
>
    <div class="mod-image">
        <img
            src={mod.image}
            alt={mod.title}
            draggable="false"
        />

        <div class="tags">
            <!-- <span class="tag updated"> -->
            <!-- 	<Clock size={13} /> -->
            <!-- 	{mod.lastUpdated} -->
            <!-- </span> -->
        </div>
    </div>

    <div class="mod-info">
        <h3>{mod.title}</h3>
        <p>{truncateText(stripMarkdown(mod.description))}</p>
    </div>

    <div class="button-container">
        <button
            class="download-button"
            class:installed={$installationStatus[mod.title]}
            disabled={$installationStatus[mod.title] ||
                $loadingStates[mod.title]}
            onclick={installMod}
        >
            {#if $loadingStates[mod.title]}
                <div class="spinner"></div>
            {:else}
                <Download size={18} />
                {$installationStatus[mod.title]
                    ? "Installed"
                    : "Download"}
            {/if}
        </button>
		
        {#if $installationStatus[mod.title]}
            <button
                class="delete-button"
                title="Remove Mod"
                onclick={uninstallMod}
            >
                <Trash2 size={18} />
            </button>
        {/if}
    </div>
</div>

<style>

    .mod-card {
        --bg-color: var(--orig-color1, #4f6367);
        --bg-color-2: var(--orig-color2, #334461);

        display: flex;
        flex-direction: column;
        position: relative;
        border-radius: 8px;
        overflow: hidden;
        border: 2px solid #f4eee0;
        width: 300px;
        max-width: 300px;
        height: 330px;
        margin: 0 auto;
        padding: 1rem;
        box-sizing: border-box;
        cursor: pointer;
        background-size: 100% 200%;
        transition: all 0.3s ease;
        background-image: repeating-linear-gradient(
            -45deg,
            var(--bg-color),
            var(--bg-color) 10px,
            var(--bg-color-2) 10px,
            var(--bg-color-2) 20px
        );
    }

    .mod-card:hover {
        animation: stripe-slide-up 1.5s linear infinite;
        scale: 1.05;
    }

    @keyframes stripe-slide-up {
        0% {
            background-position: 0 0;
        }
        100% {
            background-position: 0 -55px;
        }
    }

    .mod-image {
		position: relative;
		height: 150px;
	}

	.mod-image img {
		width: 100%;
		height: 100%;
		border-radius: 5px;
		object-fit: cover;
	}

	.tags {
		position: absolute;
		top: 7.2rem;
		right: 0.35rem;
		display: flex;
		gap: 0.5rem;
	}

	.mod-info {
		flex: 1;
		padding: 0.5rem;
		position: relative;
		bottom: 1rem;
	}

	.mod-info > p {
		-webkit-line-clamp: 2;
		line-clamp: 2;
		overflow: hidden;
		display: -webkit-box;
		-webkit-box-orient: vertical;
		padding: 0 0.1rem;
	}

	.mod-info h3 {
		color: #fdcf51;
		font-size: 1.5rem;
		margin-bottom: 0.2rem;
	}

	.mod-info p {
		color: #f4eee0;
		font-size: 1rem;
		line-height: 1.2;
	}

	.button-container {
		display: flex;
		gap: 0.5rem;
		position: absolute;
		bottom: 1rem;
		left: 1rem;
		width: calc(100% - 2rem);
	}

	.download-button {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 0.5rem;
		padding: 0.75rem;
		background: #56a786;
		color: #f4eee0;
		border: none;
		outline: #459373 solid 2px;
		border-radius: 4px;
		font-family: "M6X11", sans-serif;
		font-size: 1rem;
		cursor: pointer;
		transition: all 0.2s ease;
	}

	.download-button:hover:not(.installed) {
		background: #63b897;
		transform: translateY(-2px);
	}

	.download-button.installed {
		background: #808080;
		outline-color: #666666;
		cursor: not-allowed;
	}

	.download-button:active:not(.installed) {
		transform: translateY(1px);
	}

	.delete-button {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0.75rem;
		background: #c14139;
		color: #f4eee0;
		border: none;
		outline: #a13029 solid 2px;
		border-radius: 4px;
		cursor: pointer;
		transition: all 0.2s ease;
	}

    .download-button:disabled {
		opacity: 0.8;
		cursor: not-allowed;
	}

	@media (max-width: 1160px) {
		.mod-card {
			width: 100%;
		}
	}

</style>