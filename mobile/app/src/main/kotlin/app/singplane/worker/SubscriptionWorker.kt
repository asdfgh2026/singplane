package app.singplane.worker

import android.content.Context
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import app.singplane.SingPanelApp
import java.util.concurrent.TimeUnit

class SubscriptionWorker(
    context: Context,
    params: WorkerParameters,
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result {
        val app = applicationContext as? SingPanelApp ?: return Result.failure()
        val plane = app.controlPlane
        val profiles = plane.profiles.value.filter { it.sourceType == "url" && !it.url.isNullOrBlank() }

        for (profile in profiles) {
            runCatching {
                plane.refreshProfile(profile.id)
            }.onFailure { err ->
                plane.upsertProfile(profile.copy(lastError = err.message ?: "自动更新失败"))
            }
        }
        return Result.success()
    }

    companion object {
        const val WORK_NAME = "subscription_periodic_update"

        fun schedule(context: Context, intervalMinutes: Int) {
            val wm = WorkManager.getInstance(context)
            if (intervalMinutes <= 0) {
                wm.cancelUniqueWork(WORK_NAME)
                return
            }

            val effectiveMinutes = if (intervalMinutes < 15) 15L else intervalMinutes.toLong()
            val constraints = Constraints.Builder()
                .setRequiredNetworkType(NetworkType.CONNECTED)
                .build()

            val req = PeriodicWorkRequestBuilder<SubscriptionWorker>(effectiveMinutes, TimeUnit.MINUTES)
                .setConstraints(constraints)
                .build()

            wm.enqueueUniquePeriodicWork(
                WORK_NAME,
                ExistingPeriodicWorkPolicy.UPDATE,
                req,
            )
        }
    }
}
