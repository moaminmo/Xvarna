using System.Drawing;
using System.Globalization;
using Grasshopper.Kernel;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Setup;

/// <summary>Configures and observes the shared bounded analysis scheduler.</summary>
public sealed class SchedulerComponent : GH_Component
{
    private AnalysisSchedulerOptions? applied;

    /// <summary>Initializes the scheduler component.</summary>
    public SchedulerComponent()
        : base(
            "XVARNA Scheduler",
            "XV Scheduler",
            "Configures the shared bounded worker pool and reports queue, cancellation, stale-result, and failure telemetry.",
            "XVARNA",
            "00 Setup")
    {
    }

    /// <inheritdoc />
    public override Guid ComponentGuid => new("9cab45d3-0324-45de-b860-35d7027c29cb");

    /// <inheritdoc />
    public override GH_Exposure Exposure => GH_Exposure.primary;

    /// <inheritdoc />
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    /// <inheritdoc />
    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddIntegerParameter("Concurrency", "N", "Maximum analyses executing concurrently; 0 keeps the current policy.", GH_ParamAccess.item, 0);
        input.AddIntegerParameter("Queue Capacity", "Q", "Maximum queued analyses.", GH_ParamAccess.item, 256);
        input.AddBooleanParameter("Apply", "Apply", "Apply changed limits when the scheduler is idle.", GH_ParamAccess.item, false);
    }

    /// <inheritdoc />
    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddIntegerParameter("Running", "Run", "Analyses currently executing.", GH_ParamAccess.item);
        output.AddIntegerParameter("Queued", "Queue", "Analyses waiting for a bounded slot.", GH_ParamAccess.item);
        output.AddNumberParameter("Completed", "Done", "Successfully published analyses.", GH_ParamAccess.item);
        output.AddNumberParameter("Cancelled", "Cancel", "Cooperatively cancelled analyses.", GH_ParamAccess.item);
        output.AddNumberParameter("Stale", "Stale", "Results suppressed after newer input generations.", GH_ParamAccess.item);
        output.AddNumberParameter("Failed", "Fail", "Analyses that raised errors.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Info", "Scheduler policy, live load, outcomes, and average queue latency.", GH_ParamAccess.item);
        output.AddTextParameter("Job IDs", "ID", "Live queued/running job identities.", GH_ParamAccess.list);
        output.AddTextParameter("Job Names", "Name", "Live analysis names aligned with Job IDs.", GH_ParamAccess.list);
        output.AddTextParameter("Job States", "State", "Queued or running state aligned with Job IDs.", GH_ParamAccess.list);
        output.AddNumberParameter("Progress", "%", "Live completion fraction from zero through one.", GH_ParamAccess.list);
        output.AddTextParameter("Phases", "Phase", "Live phase/cancellation diagnostics.", GH_ParamAccess.list);
    }

    /// <inheritdoc />
    protected override void SolveInstance(IGH_DataAccess dataAccess)
    {
        int concurrency = 0;
        int queueCapacity = 256;
        bool apply = false;
        dataAccess.GetData(0, ref concurrency);
        dataAccess.GetData(1, ref queueCapacity);
        dataAccess.GetData(2, ref apply);
        try
        {
            AnalysisSchedulerStatistics before = XvarnaAnalysisScheduler.GetStatistics();
            AnalysisSchedulerOptions requested = new(
                concurrency == 0 ? before.MaximumConcurrency : concurrency,
                queueCapacity);
            if (apply && applied != requested)
            {
                XvarnaAnalysisScheduler.Configure(requested);
                applied = requested;
            }
            AnalysisSchedulerStatistics stats = XvarnaAnalysisScheduler.GetStatistics();
            IReadOnlyList<AnalysisProgress> jobs = XvarnaAnalysisScheduler.GetActiveJobs();
            dataAccess.SetData(0, stats.RunningJobs);
            dataAccess.SetData(1, stats.QueuedJobs);
            dataAccess.SetData(2, (double)stats.CompletedJobs);
            dataAccess.SetData(3, (double)stats.CancelledJobs);
            dataAccess.SetData(4, (double)stats.StaleJobs);
            dataAccess.SetData(5, (double)stats.FailedJobs);
            dataAccess.SetData(6, string.Create(
                CultureInfo.InvariantCulture,
                $"Policy: {stats.MaximumConcurrency:N0} concurrent; {stats.MaximumQueuedJobs:N0} queued maximum{Environment.NewLine}" +
                $"Live: {stats.RunningJobs:N0} running; {stats.QueuedJobs:N0} queued{Environment.NewLine}" +
                $"Outcomes: {stats.CompletedJobs:N0} completed; {stats.CancelledJobs:N0} cancelled; {stats.StaleJobs:N0} stale; {stats.FailedJobs:N0} failed{Environment.NewLine}" +
                $"Average queue wait: {stats.AverageQueueMilliseconds:N3} ms"));
            dataAccess.SetDataList(7, jobs.Select(job => job.JobId.ToString("D")));
            dataAccess.SetDataList(8, jobs.Select(job => job.Name));
            dataAccess.SetDataList(9, jobs.Select(job => job.State.ToString()));
            dataAccess.SetDataList(10, jobs.Select(job => job.Fraction));
            dataAccess.SetDataList(11, jobs.Select(job => job.CancellationRequested ? $"{job.Phase} (cancelling)" : job.Phase));
        }
        catch (Exception exception) when (exception is ArgumentException or InvalidOperationException)
        {
            AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message);
        }
    }
}
